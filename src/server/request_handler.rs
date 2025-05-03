use crate::handlers::shell::start_shell;
use crate::handlers::status::{check_power, check_storage, get_status};
use crate::settings::{self, get_or_create_settings, load_settings, save_settings, Settings};
use system_manager_server::auth::{auth_user, is_sudo};

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::task;

use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::{accept_hdr_async, tungstenite::Message, WebSocketStream};
use users::User;
fn auth(auth_string: &str) -> Option<User> {
    if let Some((username, password)) = auth_string.split_once(':') {
        match auth_user(username, password) {
            Ok(user) => Some(user),
            Err(error) => {
                eprintln!("Error with login handshake: {}", error);
                None
            }
        }
    } else {
        None
    }
}

async fn path_handler<S>(
    mut ws: WebSocketStream<S>,
    user: User,
    path: String,
) 
where S: AsyncRead + AsyncWrite + Unpin + Send + 'static

{
    println!("WebSocket session started on path: {}", path);
    match path.as_str(){
        "/shell" => {
            if let Some(socket) = start_shell(user, ws).await{
                ws = socket;
            }else{
                return;
            }
        }
        "/status" => {
            let result = task::spawn_blocking(get_status).await;
            if let Ok(Ok(status)) = result {
                let _ = ws.send(Message::Text(status.into())).await;
            } else if let Ok(Err(error)) = result {
                let _ = ws.send(Message::Text(format!("ERROR: invalid status {}",error).into())).await;
            }else{
                let error = result.err().unwrap();
                eprintln!("{}",error);
            }
        }
        "/settings/pull" =>{
                let current_settings = match task::spawn_blocking(get_or_create_settings).await{
                Ok(Ok(value))=> value,
                Ok(Err(error))=> {
                    let _ = ws.send(Message::Text(format!("ERROR: failed to load settings {}",error).into())).await;
                    return;

                },
                Err(error) =>{
                    eprintln!("{}",error);
                    return;

                }
            };
            let _ = ws.send(serde_json::to_string(&current_settings).unwrap().into()).await; // This
                                                                                        // should
                                                                                        // not
                                                                                        // throw a
                                                                                        // error
        }
        "/settings/push"=>{
            if let Some(Ok(Message::Text(j_string))) = ws.next().await{ // This should be the case
                if is_sudo(user){
                    let settings:Settings = match task::spawn_blocking(move || serde_json::from_str(&j_string)).await{
                        Ok(Ok(value)) => value,
                        Ok(Err(error))=>{
                            let _ = ws.send(format!("ERROR: failed to set new settings due to error {}",error).into()).await;
                            return;
                        }
                        Err(error)=> {
                            eprintln!("{}",error);
                            return;

                        }

                    };
                    match task::spawn_blocking(move || save_settings(&settings)).await{
                        Ok(Ok(_))=> {
                            let _ = ws.send("SAVED".into()).await;
                        },
                        Ok(Err(error)) => {
                            let _ = ws.send(format!("ERROR: failed to save settings due to error {}",error).into()).await;
                        }
                        Err(error)=>{
                            eprintln!("{}",error);
                        }
                    }
                }else{
                    let _ = ws.send("ERROR: You don't have permisson to change settings please contact your system admin".into()).await;
                }

            }

        }
        "/status/power"=>{
            let current_power = match task::spawn_blocking(check_power).await{
                Ok(Ok(value))=> value,
                Err(error)=>{
                    eprintln!("{}",error);
                    return;
                },
                Ok(Err(error))=>{
                    let _ = ws.send("ERROR: Failed to check power".into()).await;
                    return;
                }
            };
            let j_string = match task::spawn_blocking(move ||serde_json::to_string(&current_power)).await{
                Ok(Ok(value))=>value,
                _ =>{
                    let _ = ws.send("ERROR: Failed to check power".into()).await;
                        return;
                }
            };
            let _ = ws.send(j_string.into()).await;
        }
        "/status/storage"=>{
            let current_storage = match task::spawn_blocking(check_storage).await{
                Ok(value)=> value,
                Err(error)=>{
                    eprintln!("{}",error);
                    return;
                },
            };
            let j_string = match task::spawn_blocking(move ||serde_json::to_string(&current_storage)).await{
                Ok(Ok(value))=>value,
                _ =>{
                    let _ = ws.send("ERROR: Failed to check storage".into()).await;
                        return;
                }
            };
            let _ = ws.send(j_string.into()).await;
        }
        path if path.starts_with("/plugin/") => {
            todo!()
        }
        path if path.starts_with("/update/") => {
            todo!()
        }
        path if path.starts_with("/manage/") => {
            todo!()
        }
        _ => {
            eprintln!("Invalid path: {}", path);
            let _ = ws
                .send(Message::Text("ERROR: invalid request".into()))
                .await;
        }
    }
    let _ = ws.send(Message::Text("Bye".into())).await;
    
}

pub async fn handle_connection<S>(mut raw_stream: S) 
where S: AsyncRead + AsyncWrite + Unpin + Send + 'static
{
    let mut ws_path:String = String::from("/");
    // Extract path during handshake
    let callback = |req: &Request, response: Response| {
        if let Some(uri) = req.uri().path_and_query() {
            ws_path = uri.path().to_string();
            println!("WebSocket requested path: {}", ws_path);
        }
        Ok(response)
    };

    let ws_stream = match accept_hdr_async(raw_stream, callback).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("WebSocket handshake failed: {}", e);
            return;
        }
    };
    let (mut write, mut read) = ws_stream.split();

    // Receive authentication string
    let auth_msg = match read.next().await {
        Some(Ok(Message::Text(auth_string))) => auth_string,
        _ => {
            eprintln!("Failed to receive auth string");
            return;
        }
    };
    let user = match auth(&auth_msg) {
        Some(user) => user,
        None => {
            let _ = write
                .send(Message::Text("ERROR: invalid user".into()))
                .await;
            return;
        }
    };

    let _ = write.send(Message::Text("READY".into())).await;
    // Recombine write/read for full-duplex handling
    let ws = match write.reunite(read){
        Ok(value) => value,
        Err(error) => {
            eprintln!("Failed to reunite websocket");
            return;
        }
    };
    path_handler(ws, user, ws_path).await;
}

