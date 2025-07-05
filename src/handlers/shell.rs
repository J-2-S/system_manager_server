use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use libc::{getegid, geteuid, getgroups, gid_t, seteuid, setgroups};
use std::{process::Stdio, ptr};
use system_manager_server::auth::USER_LOCK;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    process::Command,
    sync::mpsc,
    task,
};
use users::{User, os::unix::UserExt};

pub async fn start_shell(mut socket: WebSocket, user: User) {
    let lock = USER_LOCK.lock().await;

    let original_uid = unsafe { geteuid() };
    let original_gid = unsafe { getegid() };

    // Save original groups
    let groups_len = unsafe { getgroups(0, ptr::null_mut()) };
    let mut original_groups: Vec<gid_t> = vec![0; groups_len as usize];
    unsafe {
        getgroups(groups_len, original_groups.as_mut_ptr());
    }

    // Switch to target user
    if let Some(groups) = user.groups() {
        let group_ids: Vec<gid_t> = groups.iter().map(|g| g.gid() as gid_t).collect();
        unsafe {
            setgroups(group_ids.len(), group_ids.as_ptr());
        }
    } else {
        unsafe {
            setgroups(0, ptr::null());
        }
    }
    unsafe {
        seteuid(user.uid());
    }

    // Prepare shell command
    let shell = user.shell();
    let shell_name = shell.file_name().unwrap_or_default();
    let mut command = Command::new(shell);

    if shell_name == "bash" {
        command.arg("--login");
    }

    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    match command.spawn() {
        Ok(mut child) => {
            let mut stdout = BufReader::new(child.stdout.take().unwrap());
            let mut stderr = BufReader::new(child.stderr.take().unwrap());
            let mut stdin = BufWriter::new(child.stdin.take().unwrap());

            let (shell_tx, mut shell_rx) = mpsc::channel::<Vec<u8>>(8);

            // Task: forward shell output to socket sender
            task::spawn(async move {
                let mut out_buf = vec![0; 1024];
                let mut err_buf = vec![0; 1024];

                loop {
                    tokio::select! {
                        read = stdout.read(&mut out_buf) => {
                            match read {
                                Ok(0) | Err(_) => break,
                                Ok(n) => {
                                    let _ = shell_tx.send(out_buf[..n].to_vec()).await;
                                }
                            }
                        }
                        read = stderr.read(&mut err_buf) => {
                            match read {
                                Ok(0) | Err(_) => break,
                                Ok(n) => {
                                    let _ = shell_tx.send(err_buf[..n].to_vec()).await;
                                }
                            }
                        }
                    }
                }
            });

            // Task: bidirectional WebSocket <-> Shell
            let _ = task::spawn(async move {
                loop {
                    tokio::select! {
                        Some(Ok(msg)) = socket.next() => {
                            match msg {
                                Message::Binary(data) => {
                                    if stdin.write_all(&data).await.is_err() {
                                        break;
                                    }
                                    let _ = stdin.flush().await;
                                }
                                Message::Close(_) | Message::Text(_) | Message::Ping(_) | Message::Pong(_) => {
                                    break;
                                }
                            }
                        }
                        Some(data) = shell_rx.recv() => {
                            let _ = socket.send(Message::binary(data)).await;
                        }
                        else => break,
                    }
                }
            }).await;

            // Wait for shell exit
            let _ = child.wait().await;
        }

        Err(e) => {
            let _ = socket
                .send(Message::text(format!(
                    "ERROR: failed to start shell: {}",
                    e
                )))
                .await;
        }
    }

    // Revert permissions
    unsafe {
        seteuid(original_uid);
        setgroups(groups_len as usize, original_groups.as_mut_ptr());
    }

    drop(lock);
}
