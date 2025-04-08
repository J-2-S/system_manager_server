use std::{os::unix::process::CommandExt, process::{Command, Stdio}};
use system_manager_server::*;
use libc::{setgid, setuid};
use users::os::unix::UserExt;
#[tokio::main]
async fn main() {
}
