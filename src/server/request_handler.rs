
use std::str::FromStr;
use serde::{Serialize,Deserialize};
use serde_json;
use users::User;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}, task};


use crate::auth::auth_user;
// Handles incoming socket data: reads the message size and message content
/// This is all the options we will use
#[derive(Debug,Serialize,Deserialize)]
enum Options{
    Shell,
    Update,
    Status,
    Module //This might not be used till match later on

}

fn first_handshake(auth_string:&str)->Option<User>{
    let mut value = auth_string.splitn(2, ":");
    let username = value.next()?;
    let password = value.next()?;
    match auth_user(username, password){
        Ok(user)=>Some(user),
        Err(error)=>{
            eprintln!("Error with login handshake :{}",error);
            None
        }
    }

}

pub async fn socket_handle<S>(mut socket: S) 
    where S: AsyncReadExt + AsyncWriteExt +Unpin,
{
    let mut buf:Vec<u8> = vec![0,1024];
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
    let string_data = match String::from_utf8(buf){
        Ok(value) => value,
        Err(error)=>{
            eprintln!("invaild auth_string {}",error);
            return;
            
        }
    };
    let user = first_handshake(&string_data).unwrap_or({
        eprintln!("ERROR invaild user ERROR");
        return;
    });


}



