use std::{net::SocketAddr, path::Path};

mod update_manager;
mod server;
mod settings;
mod handlers;
use system_manager_server::load_plugins;
#[tokio::main]
async fn main() {
    load_plugins().await;
    server::start(Path::new("dev.key"), Path::new("dev.crt"),"10.0.0.131:6969").await.unwrap();
}
