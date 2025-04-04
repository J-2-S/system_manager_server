
use std::string;
use serde::{Serialize,Deserialize};
use serde_json;
use users::User;
use tokio::{io::{self, AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}, task};
#[derive(Serialize,Deserialize,Debug)]
struct Payload{
    path:String,
    content:String,
}
enum HandleError {
    StringError(string::FromUtf8Error),
    IOError(io::Error),
}
use crate::auth::auth_user;
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
async fn path_handler<S>(mut socket:S,payload:Payload,user:User)
    where S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    if payload.path.to_lowercase() == "/shell"{
        todo!() //write the code to handle the shell and pass the socket
    }else if payload.path.to_lowercase() == "/status" {
        todo!() // write the code to handle the status
    }else if payload.path.starts_with("/plugin/"){
        todo!() //write the code to handle the plugin
    }else if payload.path.to_lowercase() == "/update"{
        todo!() //write the code to handle the update logic
    }else{
        eprintln!("Error invaild path {}",payload.path);
        let _ = socket.write(b"ERROR: invaild request").await;
    }




}
    pub async fn first_handshake<S>(mut socket: S) 
    where S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    let mut buf:Vec<u8> = vec![0;1024];
    let size = match socket.read(&mut buf).await{
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
    let size = match socket.read(&mut buf).await{
        Ok(size) => size,
        Err(error) =>{
            eprintln!("Error reading from socket: {}",error);
            0
        }
    };
    let data = match String::from_utf8(buf){
        Ok(value)=> value,
        Err(error)=>{
            eprintln!("Error invaild request string: {}",error);
            let _ = socket.write(b"ERROR: invaild json payload").await;
                return;

        }
    
    };
    let payload:Payload = match serde_json::from_str(&data){
        Ok(value) => value,
        Err(error) => {
            eprintln!("Error invaild payload string {}",error);
            let _ = socket.write(b"ERROR: invaild json payload").await;
            return;
        }
    };

    path_handler(socket, payload, user).await;

}



