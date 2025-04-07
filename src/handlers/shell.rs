use std::{io::Read, process::Stdio, sync::Arc};
use libc::{getegid, geteuid, getgroups, gid_t, seteuid, setgroups};
use rustls::lock::Mutex;
use tokio::{io::{AsyncReadExt, AsyncWriteExt,BufReader,BufWriter}, process::Command, sync::mpsc, task};
use users::{os::unix::UserExt, User};
use std::ptr;
use crate::auth::USER_LOCK;
async fn start_shell<S>(user: User, mut socket: S) -> Option<S>
where
    S: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static,
{
    let lock = USER_LOCK.lock().await;
    let original_user = unsafe { geteuid() }; // Get the original user (likely root)
    let original_group = unsafe { getegid() }; // Get the original primary group

    // Get the number of groups by calling getgroups with size 0 (to just get the count)
    let groups_len = unsafe { getgroups(0, ptr::null_mut()) };
    let mut original_groups: Vec<gid_t> = vec![0; groups_len as usize];

    // Set the user's groups if available
    if let Some(groups) = user.groups() {
        let group_ids: Vec<gid_t> = groups.iter().map(|group| group.gid() as gid_t).collect();
        unsafe {
            setgroups(group_ids.len(), group_ids.as_ptr());
        }
    } else {
        // If no groups, clear the existing groups
        unsafe {
            setgroups(0, ptr::null());
        }
    }

    // Set the user to the target user, done last to ensure root privileges until this point
    unsafe { seteuid(user.uid()) };

    // Try to spawn the child process
    let process = Command::new(user.shell())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::piped())
        .spawn();

    match process {
        Ok(mut child) => {
            // Spawn asynchronous tasks to handle I/O between the socket and the shell
            let child_stdout = child.stdout.take().unwrap();
            let child_stderr = child.stderr.take().unwrap();
            let child_stdin = child.stdin.take().unwrap();
            let (io_out, mut socket_in) = mpsc::channel::<Vec<u8>>(1);

            // Task to read from child stdout and send to the socket
            task::spawn(async move {
                let mut stdout = BufReader::new(child_stdout);
                let mut stderr = BufReader::new(child_stderr);
                let mut out_buf = Vec::new();
                let mut err_buf = Vec::new();
                loop {
                    tokio::select! {
                        result = stdout.read_buf(&mut out_buf) => {
                            match result {
                                Ok(0) => {
                                    // stdout closed
                                    break;
                                },
                                Ok(_) => {
                                    let _ = io_out.send(out_buf.clone()).await;
                                },
                                Err(e) => {
                                    eprintln!("Error reading stdout: {}", e);
                                    break;
                                }
                            }
                        }
                        result = stderr.read(&mut err_buf) => {
                            match result {
                                Ok(0) => {
                                    // stderr closed
                                    break;
                                },
                                Ok(_) => {
                                    let _ = io_out.send(err_buf.clone()).await;
                                },
                                Err(e) => {
                                    eprintln!("Error reading stderr: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                }
            });

            // Task to handle I/O between socket and child stdin
            let future = task::spawn(async move {
                let mut buf = Vec::new();
                let mut stdin = BufWriter::new(child_stdin);
                loop {
                    tokio::select! {
                        result = socket.read_buf(&mut buf) => {
                            match result {
                                Ok(0) => break,
                                Ok(_) => {
                                    let _ = stdin.write(&buf).await;
                                },
                                Err(e) => {
                                    eprintln!("Error reading from socket: {}", e);
                                    break;
                                }
                            }
                        }
                        result = socket_in.recv() => {
                            match result {
                                Some(value) => {
                                    let _ = socket.write(&value).await;
                                }
                                None => break
                            }
                        }
                    }
                }
                socket
            });

            // Wait for the child process to finish
            let status = child.wait().await.expect("Failed to wait on child process");
            println!("Child process exited with status: {}", status);

            // Restore original user and group
            unsafe { seteuid(original_user) };
            unsafe { setgroups(groups_len as u32 as usize, original_groups.as_mut_ptr()) };

            drop(lock);
            (future.await).ok()    
        }
        Err(error) => {
            eprintln!("Error: failed to open shell: {}", error);
            let _ = socket.write_all(b"ERROR: failed to open shell").await;

            // Restore original user and group
            unsafe { seteuid(original_user) };
            unsafe { setgroups(groups_len as u32 as usize, original_groups.as_mut_ptr()) };

            drop(lock);
            None
        }
    }
}
