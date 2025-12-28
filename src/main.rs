use std::sync::atomic::AtomicBool;
mod router;
mod settings;
mod status;
mod update_manager;
mod users;
static RESTART_PENDING: AtomicBool = AtomicBool::new(false);
#[tokio::main]
async fn main() {
    #[cfg(debug_assertions)]
    {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .filter(Some("tracing::span"), log::LevelFilter::Off)
            .is_test(true)
            .try_init();
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = evn_logger::init();
    }
    router::init_router().await;
}
