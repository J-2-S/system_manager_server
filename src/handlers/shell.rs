use std::{process::Stdio, ptr};
use libc::{getegid, geteuid, getgroups, gid_t, seteuid, setgroups};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    process::Command,
    sync::mpsc,
    task,
};
use users::{os::unix::UserExt, User};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use futures_util::{SinkExt, StreamExt};
use system_manager_server::auth::USER_LOCK;

pub async fn start_shell<S>(user: User, mut socket: WebSocketStream<S>) -> Option<WebSocketStream<S>>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let lock = USER_LOCK.lock().await;
    let original_user = unsafe { geteuid() };
    let original_group = unsafe { getegid() };

    let groups_len = unsafe { getgroups(0, ptr::null_mut()) };
    let mut original_groups: Vec<gid_t> = vec![0; groups_len as usize];
    unsafe {
        getgroups(groups_len, original_groups.as_mut_ptr());
    }

    if let Some(groups) = user.groups() {
        let group_ids: Vec<gid_t> = groups.iter().map(|g| g.gid() as gid_t).collect();
        unsafe { setgroups(group_ids.len(), group_ids.as_ptr()) };
    } else {
        unsafe { setgroups(0, ptr::null()) };
    }

    unsafe { seteuid(user.uid()) };

    let process = Command::new(user.shell())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    match process {
        Ok(mut child) => {
            let mut stdout = BufReader::new(child.stdout.take().unwrap());
            let mut stderr = BufReader::new(child.stderr.take().unwrap());
            let mut stdin = BufWriter::new(child.stdin.take().unwrap());

            let (io_out, mut socket_in) = mpsc::channel::<Vec<u8>>(8);

            // Task: Read shell output and forward to WebSocket
            task::spawn(async move {
                let mut out_buf = vec![0u8; 1024];
                let mut err_buf = vec![0u8; 1024];

                loop {
                    tokio::select! {
                        read = stdout.read(&mut out_buf) => {
                            if let Ok(n) = read {
                                if n == 0 { break; }
                                let _ = io_out.send(out_buf[..n].to_vec()).await;
                            } else {
                                break;
                            }
                        }
                        read = stderr.read(&mut err_buf) => {
                            if let Ok(n) = read {
                                if n == 0 { break; }
                                let _ = io_out.send(err_buf[..n].to_vec()).await;
                            } else {
                                break;
                            }
                        }
                    }
                }
            });

            // Task: Bi-directional WebSocket <-> Shell
            let forwarder = task::spawn(async move {
                loop {
                    tokio::select! {
                        msg = socket.next() => {
                            match msg {
                                Some(Ok(Message::Binary(data))) => {
                                    if stdin.write_all(&data).await.is_err() {
                                        break;
                                    }
                                    let _ = stdin.flush().await;
                                }
                                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                                _ => {}
                            }
                        }
                        msg = socket_in.recv() => {
                            if let Some(data) = msg {
                                let _ = socket.send(Message::Binary(data.into())).await;
                            } else {
                                break;
                            }
                        }
                    }
                }
                socket
            });

            let _ = child.wait().await;

            unsafe {
                seteuid(original_user);
                setgroups(groups_len as usize, original_groups.as_mut_ptr());
            }

            drop(lock);
            forwarder.await.ok()
        }

        Err(e) => {
            eprintln!("Shell error: {}", e);
            let _ = socket.send(Message::Text("ERROR: failed to open shell".into())).await;

            unsafe {
                seteuid(original_user);
                setgroups(groups_len as usize, original_groups.as_mut_ptr());
            }

            drop(lock);
            None
        }
    }
}
