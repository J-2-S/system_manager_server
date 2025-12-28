//! # System Manager Server
//!
//! This is a web server for managing a Linux system.

// Modules
mod router;
mod settings;
mod status;
mod update_manager;
mod users;

// Static variables
use std::sync::atomic::AtomicBool;
/// A flag to indicate if a restart is pending.
static RESTART_PENDING: AtomicBool = AtomicBool::new(false);

/// The main entry point of the application.
#[tokio::main]
async fn main() {
    // Initialize the logger
    #[cfg(debug_assertions)]
    {
        // In debug builds, use a more verbose logger
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .filter(Some("tracing::span"), log::LevelFilter::Off)
            .is_test(true)
            .try_init();
    }
    #[cfg(not(debug_assertions))]
    {
        // In release builds, use the default logger
        let _ = env_logger::init();
    }

    // Initialize the router
    router::init_router().await;
}
