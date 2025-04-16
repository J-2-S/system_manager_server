use crate::handlers::status::get_status;
use std::string;
use serde::{Serialize,Deserialize};
use serde_json;
use users::User;
use tokio::{io::{self, AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}, task};
use system_manager_server::auth::auth_user;
#[derive(Serialize,Deserialize,Debug)]
struct Payload{
    key:String,
    content:String,
}
// Handles incoming socket data: reads the message size and message content
/// This is all the options we will us
fn auth(auth_string:&str)->Option<User>{
    if let Some((username,password)) = auth_string.split_once( ":"){
        match auth_user(username, password){
            Ok(user)=>Some(user),
            Err(error)=>{
                eprintln!("Error with login handshake :{}",error);
                None
            }
        }
    }else{
        None
    }
}
async fn path_handler<S>(mut socket:S,user:User)
    where S: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static,
{
    
    let mut buf:Vec<u8> = Vec::new();
    let size = socket.read_buf(&mut buf).await.unwrap_or(0);
    if size == 0{
        return;
    }
    let payload:Payload = match serde_json::from_slice(&buf){
        Ok(value) => value,
        Err(_) => {
            eprintln!("Error invalid payload");
            return;
        }
    };
    if payload.key.to_lowercase() == "/shell"{
        todo!() //write the code to handle the shell and pass the socket
    } else if payload.key.to_lowercase() == "/status" {
        if let Ok(Ok(status )) = task::spawn_blocking(get_status).await {
            let message = Payload{key:"/status".to_string(), content:status };
            match serde_json::to_string(&message) {
                Ok(value)=>{
                    let result = task::spawn(async move {
                        let _ = socket.write(value.as_bytes()).await;
                        socket
                    }).await;
                    match result{
                        Ok(value)=>Box::pin(path_handler(value, user)).await,
                        Err(error)=> eprintln!("Error sending status to socket: {}",error)
                    }
                },
                Err(error)=>{
                    if let Ok(socket)= task::spawn( async move {
                        let _ = socket.write(b"ERROR: failed to create payload").await;
                        socket
                    }).await{
                        Box::pin(path_handler(socket, user)).await;

                    }
                }
            }
        } else { 
            eprintln!("Error getting status");
            let _ = socket.write(b"Error: invalid status").await;
        }
    } else if payload.key.starts_with("/plugin/") {
        todo!() //write the code to handle the plugin
    } else if payload.key.starts_with("/update/") {
        todo!() //write the code to handle the update logic
    }else if payload.key.starts_with("/manage/") {
        todo!() //write the code to handle the managment of the system
    }else if payload.key == "/bye"{
        // Should never throw a error
        let json_string = serde_json::to_string(&Payload { key: "/bye".to_string(), content:"".to_string() }).unwrap();
        
            let _ = socket.write(json_string.as_bytes()).await;
        return; // end the loop
    } else{
        eprintln!("Error invaild path {} ",payload.key);
        let _ = socket.write(b"ERROR: invaild request").await;
        Box::pin(path_handler(socket, user)).await;
    }

}
    pub async fn first_handshake<S>(mut socket: S) 
    where S: AsyncReadExt + AsyncWriteExt + Unpin + Send + 'static,
{
    let mut buf:Vec<u8> = Vec::new();
    let size = match socket.read_buf(&mut buf).await{
        Ok(size) => size,
        Err(error) =>{
            eprintln!("Error reading from socket: {}",error);
            0
        }
    };
    if size == 0{
        return;
    }
    let auth_string = match String::from_utf8(buf.clone()){
        Ok(value) => value,
        Err(error)=>{
            eprintln!("invaild auth_string {}",error);
            return;
            
        }
    };
    buf.clear();
    let user = match auth(&auth_string){
        Some(user)=> user,
        None=>{
            eprintln!("ERROR: invaild user");
            return;
        }
    };
    let _ = socket.write(b"AWAITING PAYLOAD").await;
    let size = match socket.read_buf(&mut buf).await{
        Ok(size) => size,
        Err(error) =>{
            eprintln!("Error reading from socket: {}",error);
            0
        }
    };

    path_handler(socket, user).await;

}



