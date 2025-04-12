use std::{os::unix::process::CommandExt, process::{Command, Stdio}};
mod server;
mod settings;
mod handlers;
mod api;
mod auth;
mod update_manager;
use libc::{setgid, setuid};
use users::os::unix::UserExt;
#[tokio::main]
async fn main() {
}
