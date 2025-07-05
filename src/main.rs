use std::path::Path;
mod handlers;
mod server;
mod settings;
mod templates;
mod update_manager;
#[tokio::main]
async fn main() {
    server::start(
        Path::new("dev.key"),
        Path::new("dev.crt"),
        "10.0.0.131:6969",
    )
    .await
    .unwrap();
}
