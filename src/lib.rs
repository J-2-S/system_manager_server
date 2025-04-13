use std::ffi::{CStr, CString};
use std::sync::Arc;
mod server;
mod settings;
mod handlers;
mod auth;
mod update_manager;
use libc::{c_char, uid_t};
use tokio::{sync::Mutex,task};

// C-callback type
pub type Callback = unsafe extern "C" fn(uid_t,*const c_char,usize)->*const c_char;

pub struct APICommand {
    name: String,
    function: Callback,
    needs_root: bool,
    takes_input:bool
}

#[repr(C)]
pub struct Plugin {
    name: Arc<String>,
    commands: Arc<Mutex<Vec<APICommand>>>,
}

impl Plugin {
    pub async fn call_command(&self,name:&str,message:Option<String>,user_id:uid_t)->Option<String>{
        let commands = self.commands.clone();
        for command in commands.lock().await.iter(){
            if command.name == name{
                let function = command.function;
                let result = task::spawn_blocking(move || {
                    let c_message:CString;
                    let size:usize;
                    if let Some(message) = message{
                        let clean_message:String = message.chars().filter(|&c| c!= '\0').collect();
                        size = clean_message.len();
                        c_message = CString::new(message).unwrap();
                    }else{
                        c_message = CString::default();
                        size = 0;

                    }
                    let json_string = unsafe{(function)(user_id,c_message.as_ptr(),size)};
                    if !json_string.is_null(){
                        let json_string = unsafe {CStr::from_ptr(json_string)};   
                        if let Ok(c_var) = json_string.to_str(){
                            let rust_var = c_var.to_owned();
                            Some(rust_var)
                        }else{

                            None

                        }
                    }else{
                        None
                    }

                }).await;
                match result {
                    Ok(value) => return value,
                    Err(_error) => {
                        eprintln!("Command Failed to run: {}",name);
                        return None;
                    }

                }

            }
        }
        None
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn init_command(
    plugin: *const Plugin,
    name: *const c_char,
    function: Callback,
    needs_root: bool,
    takes_input:bool
) {
    // Safety: Check null pointers
    if plugin.is_null() || name.is_null() {
        eprintln!("init_command: null pointer received.");
        return;
    }

    // Safety: Convert C string to Rust string
    let name_str = unsafe {
        match CStr::from_ptr(name).to_str() {
            Ok(s) => s.to_string(),
            Err(e) => {
                eprintln!("init_command: invalid UTF-8 in name: {}", e);
                return;
            }
        }
    };

    // Safety: Reborrow and clone Arc
    let plugin_ref = unsafe { &*plugin };

    let command = APICommand {
        name: name_str,
        function,
        needs_root,
        takes_input
    };

    let commands = plugin_ref.commands.clone();

    // Spawn a task to lock and insert the command
    // (You could await instead if this were called from async Rust)
    task::spawn(async move {
        let mut locked = commands.lock().await;
        locked.push(command);
    });
}
#[unsafe(no_mangle)]
pub extern "C" fn test_api(){
    println!("this is a test to see if I can call exe functions in a dll");

}
