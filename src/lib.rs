// === Modules ===
pub mod auth;

// === External Crates ===
use axum::{http::Method as AxumMethod, response::Response};
use futures_util::StreamExt;
use libc::{c_char, c_void, uid_t};
use once_cell::sync::OnceCell;
use std::{ffi::CStr, fs, ptr};

use directories::ProjectDirs;
use tokio::sync::mpsc;
// === Constants ===
static DIRS: OnceCell<ProjectDirs> = OnceCell::new();
pub fn dirs() -> &'static ProjectDirs {
    DIRS.get_or_init(|| ProjectDirs::from("com", "system-manager", "server").unwrap())
}
// === Public Types ===

#[repr(C)]
#[derive(Clone, Copy)]
pub enum Method {
    GET,
    POST,
    PUT,
    DELETE,
    HEAD,
    TRACE,
    CONNECT,
    PATCH,
    UNKNOWN,
}

impl From<AxumMethod> for Method {
    fn from(method: AxumMethod) -> Self {
        match method {
            GET => Method::GET,
            POST => Method::POST,
            PUT => Method::PUT,
            DELETE => Method::DELETE,
            HEAD => Method::HEAD,
            TRACE => Method::TRACE,
            CONNECT => Method::CONNECT,
            PATCH => Method::PATCH,
            _ => Method::UNKNOWN,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RequestData {
    pub method: Method,
    pub path: *const c_char,
    pub content_type: *const c_char,
    pub body: *const c_char,
    pub headers: (*const c_char, *const c_char),
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Header {
    pub key: *const c_char,
    pub value: *const c_char,
}

pub struct WebSocket {
    sender: mpsc::Sender<u8>,
    receiver: mpsc::Receiver<u8>,
}

// === Plugin Registration and Dispatch ===

pub type HttpHandler =
    unsafe extern "C" fn(*const RequestData, *const Plugin, uid_t) -> *const c_void;
pub type WebsocketHandler = unsafe extern "C" fn(*const WebSocket, *const Plugin, uid_t);

#[derive(Clone, Copy)]
#[repr(C)]
enum HandlerFunction {
    Http(HttpHandler),
    Websocket(WebsocketHandler),
}

#[derive(Clone)]
struct Handler {
    name: String,
    function: HandlerFunction,
}

pub struct Plugin {
    name: String,
    methods: Vec<Handler>,
}

pub static PLUGINS: OnceCell<Vec<Box<Plugin>>> = OnceCell::new();
type InitFn = unsafe extern "C" fn() -> *const Plugin;

#[unsafe(no_mangle)]
unsafe extern "C" fn register_plugin(
    name: *const c_char,
    methods: *const Handler,
) -> *const Plugin {
    unsafe {
        let name = CStr::from_ptr(name).to_string_lossy().into_owned();
        let methods = std::slice::from_raw_parts(methods, 1); // TODO: accept length in future
        let plugin = Box::new(Plugin {
            name,
            methods: methods.to_vec(),
        });
        Box::into_raw(plugin)
    }
}

// === WebSocket IO ===

#[unsafe(no_mangle)]
pub unsafe extern "C" fn write_socket(
    socket: *mut WebSocket,
    data: *const c_char,
    len: usize,
) -> isize {
    unsafe {
        let sender = unsafe { &(*socket).sender };
        let buf = std::slice::from_raw_parts(data as *const u8, len);
        for byte in buf {
            if sender.blocking_send(*byte).is_err() {
                return -1;
            }
        }
        buf.len() as isize
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn read_socket(
    socket: *mut WebSocket,
    buf: *mut c_char,
    len: usize,
) -> isize {
    unsafe {
        let receiver = unsafe { &mut (*socket).receiver };
        let mut rbuf = vec![0u8; len];
        let output = receiver.blocking_recv_many(&mut rbuf, len);
        if output > 0 {
            ptr::copy_nonoverlapping(rbuf.as_ptr(), buf as *mut u8, output);
        }
        output as isize
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn close_socket(socket: *mut WebSocket) {
    unsafe {
        drop(Box::from_raw(socket));
    }
}

// === Response Creation ===

#[unsafe(no_mangle)]
pub unsafe extern "C" fn create_response(
    status: u16,
    headers: *const Header,
    headers_len: usize,
    body: *const c_char,
    body_len: usize,
) -> *const c_void {
    unsafe {
        let headers = std::slice::from_raw_parts(headers, headers_len);
        let headers = headers
            .iter()
            .map(|h| {
                (
                    CStr::from_ptr(h.key).to_string_lossy().into_owned(),
                    CStr::from_ptr(h.value).to_string_lossy().into_owned(),
                )
            })
            .collect::<Vec<_>>();

        let body = std::slice::from_raw_parts(body as *const u8, body_len).to_vec();

        let mut builder = Response::builder()
            .status(status)
            .header("Content-Type", "text/html");

        for (key, value) in headers {
            builder = builder.header(&key, &value);
        }

        match builder.body(body) {
            Ok(response) => Box::into_raw(Box::new(response)) as *const c_void,
            Err(_) => std::ptr::null(),
        }
    }
}
pub fn load_plugins() -> Result<(), Box<dyn std::error::Error>> {
    if PLUGINS.get().is_some() {
        return Ok(());
    }
    let mut plugins = vec![];
    let public_dir = dirs().data_dir().join("plugins");
    if public_dir.exists() {
        for entry in fs::read_dir(public_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                unsafe {
                    let lib = libloading::Library::new(path)?;
                    let plugin = lib.get::<InitFn>(b"init_plugin")?();
                    let plugin: Box<Plugin> = Box::from_raw(plugin as *mut Plugin);
                    Box::leak(Box::new(lib));
                    plugins.push(plugin);
                }
            }
        }
    } else {
        Err("Plugin directory does not exist")?;
    }
    let _ = PLUGINS.set(plugins);

    Ok(())
}
