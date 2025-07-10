use crate::handlers::HandleError;
use axum::{
    body::Bytes,
    extract::ws::{Message, WebSocket},
};
use futures_util::{SinkExt, StreamExt};
use libc::*;
use std::{ffi::CString, io, os::unix::io::RawFd, ptr, sync::Arc};
use tokio::{
    sync::{Mutex, mpsc},
    task,
};
use users::{User, os::unix::UserExt};

unsafe fn open_pty() -> io::Result<(RawFd, String)> {
    unsafe {
        let master_fd = posix_openpt(O_RDWR | O_NOCTTY);
        if master_fd < 0 {
            return Err(io::Error::last_os_error());
        }
        if grantpt(master_fd) != 0 || unlockpt(master_fd) != 0 {
            return Err(io::Error::last_os_error());
        }
        let ptr = ptsname(master_fd);
        if ptr.is_null() {
            return Err(io::Error::last_os_error());
        }
        let slave_name = std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned();
        Ok((master_fd, slave_name))
    }
}

fn set_raw_mode(fd: RawFd) -> Result<(), HandleError> {
    unsafe {
        let mut termios = std::mem::zeroed::<libc::termios>();
        if tcgetattr(fd, &mut termios) != 0 {
            return Err("tcgetattr failed".into());
        }
        cfmakeraw(&mut termios);
        if tcsetattr(fd, TCSANOW, &termios) != 0 {
            return Err("tcsetattr failed".into());
        }
    }
    Ok(())
}

fn active_shell(user: User, slave_path: &str) -> Result<pid_t, HandleError> {
    let uid = user.uid();
    std::env::set_current_dir(user.home_dir()).unwrap(); // This is needed for the shell to find the correct PATH
    unsafe {
        let pid = fork();
        if pid < 0 {
            return Err("Failed to fork".into());
        }

        if pid == 0 {
            // === CHILD PROCESS ===
            if setsid() < 0 {
                eprintln!("setsid failed");
            }

            let slave_fd = open(slave_path.as_ptr() as *const _, O_RDWR);
            if slave_fd < 0 {
                eprintln!("Failed to open slave");
            }

            // 🔧 Set raw mode on slave
            {
                let mut termios = std::mem::zeroed::<libc::termios>();
                if tcgetattr(slave_fd, &mut termios) == 0 {
                    cfmakeraw(&mut termios);
                    tcsetattr(slave_fd, TCSANOW, &termios);
                }
            }

            if ioctl(slave_fd, TIOCSCTTY, 0) < 0 {
                eprintln!("ioctl TIOCSCTTY failed");
            }

            dup2(slave_fd, 0);
            dup2(slave_fd, 1);
            dup2(slave_fd, 2);

            if setuid(uid) < 0 {
                eprintln!("setuid failed");
            }

            for group in user.groups().unwrap_or_default() {
                if setgid(group.gid()) < 0 {
                    eprintln!("setgid failed");
                }
            }

            let shell = CString::new(user.shell().to_string_lossy().as_bytes()).unwrap();
            let argv = [shell.as_ptr(), ptr::null()];
            execvp(argv[0], argv.as_ptr());

            std::process::exit(1); // exec failed
        }

        Ok(pid)
    }
}

fn check_exited(pid: pid_t) -> bool {
    let mut status = 0;
    unsafe { waitpid(pid, &mut status, WNOHANG) > 0 }
}

pub fn start_shell(socket: WebSocket, user: User) -> Result<(), HandleError> {
    let (master_fd, slave_path) =
        unsafe { open_pty().map_err(|e| HandleError::from(e.to_string()))? };

    // Set raw mode on master FD
    set_raw_mode(master_fd)?;

    let pid = active_shell(user, &slave_path)?;

    unsafe {
        let flags = fcntl(master_fd, F_GETFL);
        fcntl(master_fd, F_SETFL, flags | O_NONBLOCK);
    }

    let mut buf = vec![0u8; 1024];
    let (sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::channel(100);

    task::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Binary(data) => {
                    if tx.send(data.to_vec()).await.is_err() {
                        break;
                    }
                }
                Message::Text(data) => {
                    if tx.send(data.as_bytes().to_vec()).await.is_err() {
                        break;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    let sender = Arc::new(Mutex::new(sender));

    loop {
        if check_exited(pid) {
            println!("Process exited");
            break;
        }

        let n = unsafe { read(master_fd, buf.as_mut_ptr() as *mut _, buf.len()) };

        if n > 0 {
            let data = Bytes::copy_from_slice(&buf[..n as usize]);
            let sender = sender.clone();
            task::spawn(async move {
                print!("{}", String::from_utf8_lossy(&data.to_vec()));
                if let Err(e) = sender.lock().await.send(Message::Binary(data)).await {
                    eprintln!("WebSocket send error: {e}");
                }
            });
        } else if n == -1 {
            let err = unsafe { *__errno_location() };
            if err != EAGAIN && err != EWOULDBLOCK {
                eprintln!("read failed: {}", io::Error::last_os_error());
                break;
            }
        }

        if let Ok(data) = rx.try_recv() {
            let written = unsafe { write(master_fd, data.as_ptr() as *const _, data.len()) };
            if written == -1 {
                eprintln!("write failed: {}", io::Error::last_os_error());
                break;
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    Ok(())
}
