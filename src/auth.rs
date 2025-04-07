use once_cell::sync::Lazy;
use pam::{self, PamError,PamReturnCode, PasswordConv};
use tokio::sync::Mutex;
use std::{error::Error, fmt, os::unix::process::CommandExt, process::{Command, Stdio}};
use users::{User,self};
pub static USER_LOCK:Lazy<Mutex<()>> = Lazy::new(||Mutex::new(()));
#[derive(Debug)]
pub enum AuthenticateError {
    Invaild,
    PamError(PamError)
}
impl Error for AuthenticateError {
    
}
impl fmt::Display for AuthenticateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self{
        Self::Invaild => write!(f,"Invaild username or password"),
        Self::PamError(error) => write!(f,"Authenticate error: {}",error)
        }
    }
}
impl AuthenticateError {
    fn is_invaild(&self)-> bool{
        matches!(self,Self::Invaild)
    }
}
impl From<PamError> for AuthenticateError {
    fn from(value: PamError) -> Self {
        match value.0 {
            PamReturnCode::User_Unknown => AuthenticateError::Invaild,
            PamReturnCode::Auth_Err => AuthenticateError::Invaild,
            _=> AuthenticateError::PamError(value)

        }
    }
}


pub fn auth_user(username:&str,password:&str)->Result<User,AuthenticateError>{
    let mut auth = pam::Client::with_password("common-auth")?;
    auth.conversation_mut().set_credentials(username, password);
    auth.authenticate()?;
    // The user should exist us we just auth with pam
    let user = users::get_user_by_name(username).unwrap();
    Ok(user)
}
pub fn is_sudo(user:User)->bool{
    match user.groups(){
        Some(groups)=>{
            for group in groups{
                if group.name() == "sudo" || group.name() == "wheel"{
                    return true;
                }
            }
            false
        }
        None => false
    }

}
