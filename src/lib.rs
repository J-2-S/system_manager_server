


use std::collections::HashMap;
use std::ffi::{c_void, CStr, CString};
use std::sync::{Arc, OnceLock};

use libc::{c_char, uid_t};
use libloading::{Error, Library};
use tokio::{sync::Mutex, task};

pub mod auth;

/// C Types
pub type Callback = unsafe extern "C" fn(uid_t, *const c_char, usize) -> *const c_char;

/// Paths used for the program
//pub const PLUGIN_DIR: &str = "/home/linuxman/code/system_manager_server/plugins/"; //this is temp                                                                                   //for debugging
pub const CONFIG_DIR: &str = "/etc/system_manager_server/";
pub const DATA_DIR: &str = "/var/lib/system_manager_server/";
pub const CACHE_DIR: &str = "/var/cache/system_manager_server/";
pub const LOG_DIR: &str = "/var/log/system_manager_server/";
pub const PLUGIN_DIR:&str = "/usr/lib/system_manager_server/";

/// The vector used to store all the plugins and keep them in scope
pub static PLUGINS: OnceLock<Mutex<Vec<Arc<Plugin>>>> = OnceLock::new();

/// The command type
struct APICommand {
    function: Callback,
    needs_root: bool,
    takes_input: bool,
}

pub struct Plugin {
    name: Arc<String>,
    commands: Arc<Mutex<HashMap<String, APICommand>>>,
    lib: Arc<Library>,
}

impl Plugin {
    pub fn new(name: &str) -> Result<Arc<Self>, Error> {
        let lib = unsafe { Library::new(format!("{}{}", PLUGIN_DIR, name)) }?;
        let plugin = Arc::new(Self {
            name: Arc::new(name.to_string()),
            commands: Arc::new(Mutex::new(HashMap::new())),
            lib: Arc::new(lib),
        });

        // SAFETY: Plugin must be passed to the plugin's start function as a raw pointer
        unsafe {
            let start_plugin = plugin
                .lib
                .get::<unsafe extern "C" fn(*const Plugin)>(b"plugin_start")?;
            let plugin_ptr = Arc::into_raw(plugin.clone());
            start_plugin(plugin_ptr);
            Arc::from_raw(plugin_ptr); // To prevent memory leak, restore Arc (no double-free)
        }

        Ok(plugin)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub async fn call_command(
        &self,
        name: &str,
        message: &str,
        user_id: uid_t,
    ) -> Option<String> {
        let commands = self.commands.lock().await;
        let command = commands.get(name)?;

        let function = command.function;
        let message = message.to_owned();

        task::spawn_blocking(move || {
            let (c_message, size) = if !message.is_empty() {
                let clean = message.chars().filter(|&c| c != '\0').collect::<String>();
                let c_str = CString::new(clean).ok()?;
                let len = c_str.as_bytes().len();
                (c_str, len)
            } else {
                (CString::new("").unwrap(), 0)
            };

            let out_ptr = unsafe { function(user_id, c_message.as_ptr(), size) };
            if out_ptr.is_null() {
                return None;
            }

            let out_cstr = unsafe { CStr::from_ptr(out_ptr) };
            let out = out_cstr.to_str().ok()?.to_string();

            unsafe { libc::free(out_ptr as *mut c_void) };

            Some(out)
        })
        .await
        .ok()?

    }
}

/// # Safety
/// This function is unsafe because it dereferences raw pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn create_command(
    plugin: *const Plugin,
    name: *const c_char,
    function: Callback,
    needs_root: bool,
    takes_input: bool,
) {
    if plugin.is_null() || name.is_null() {
        eprintln!("init_command: null pointer received.");
        return;
    }

    let name_str = match CStr::from_ptr(name).to_str() {
        Ok(s) => s.to_string(),
        Err(e) => {
            eprintln!("init_command: invalid UTF-8: {}", e);
            return;
        }
    };

    let plugin_ref =unsafe { &*plugin};
    let commands = plugin_ref.commands.clone();
    task::spawn(async move {
        let mut locked = commands.lock().await;
        locked.insert(
            name_str,
            APICommand {
                function,
                needs_root,
                takes_input,
            },
        );
    });
}

pub async fn load_plugins() {
    let plugin_dir = match std::fs::read_dir(PLUGIN_DIR) {
        Ok(val) => val,
        Err(err) => {
            eprintln!("Could not read plugin directory: {}", err);
            return;
        }
    };

    let plugins = PLUGINS.get_or_init(|| Mutex::new(Vec::new()));
    let mut locked_plugins = plugins.lock().await;

    for entry in plugin_dir {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if path.is_file() {
                    if let Some(name) = path.file_name() {
                        let name = name.to_str().unwrap().to_owned(); //It should almost always be UTF-8
                                                                 //but we might actual need to
                                                                 //error check it
                        let _name = name.clone();
                        match task::spawn_blocking(move ||Plugin::new(&_name)).await {
                            Ok(Ok(plugin)) => locked_plugins.push(plugin),
                            Ok(Err(e)) => eprintln!("Failed to load plugin '{}': {}",name,e),
                            Err(e) => eprintln!("Failed to load plugin '{}': {}", name, e),
                       }
                    }
                }
            }
            Err(e) => eprintln!("Plugin dir entry error: {}", e),
        }
    }
}
