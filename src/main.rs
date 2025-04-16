mod update_manager;
mod server;
mod settings;
mod handlers;
#[tokio::main]
async fn main() {
    system_manager_server::load_plugins().await;
}
