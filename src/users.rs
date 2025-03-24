
use bcrypt::{hash,verify,DEFAULT_COST};
pub struct User{
    username:String,

}
impl User {
    pub fn create_user(username:String,password:String)->Self{
        let hashed = 
        Self { username }
    }
    
}
