use std::{os::unix::process::CommandExt, process::{Command, Stdio}};
mod handlers;
mod auth;
mod update_manager;
mod settings;
mod server;
use libc::{setgid, setuid};
use users::os::unix::UserExt;
#[tokio::main]
async fn main() {

}
