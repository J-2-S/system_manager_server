use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::{RESTART_PENDING, router::templates::SettingsTemplate};
#[cfg(not(debug_assertions))]
const SETTINGS_PATH: &str = "/var/lib/system_manager_server/settings.toml";
#[cfg(debug_assertions)]
const SETTINGS_PATH: &str = "./settings.toml";

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Settings {
    pub port: u16,
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
    pub hostname: String,
    pub ignore_updates: bool,
    pub threatsholds: Threasholds,
}
impl Settings {
    pub fn secure(&self) -> bool {
        self.cert_path.exists() && self.key_path.exists()
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Threasholds {
    pub low_power: u8,
    pub low_storage: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            port: 8080,
            cert_path: PathBuf::from("/etc/system_manager_server/cert.pem"),
            key_path: PathBuf::from("/etc/system_manager_server/key.pem"),
            hostname: String::from("0.0.0.0"),
            ignore_updates: false,
            threatsholds: Threasholds::default(),
        }
    }
}

impl Default for Threasholds {
    fn default() -> Self {
        Self {
            low_power: 15,
            low_storage: 15,
        }
    }
}
impl Into<SettingsTemplate> for Settings {
    fn into(self) -> SettingsTemplate {
        SettingsTemplate {
            low_storage: self.threatsholds.low_storage,
            low_power: self.threatsholds.low_power,
            ignore_update: self.ignore_updates,
            cert_path: self.cert_path.to_string_lossy().to_string(),
            key_path: self.key_path.to_string_lossy().to_string(),
            port: self.port,
            hostname: self.hostname.to_string(),
        }
    }
}
pub async fn load_settings() -> Settings {
    let content = match fs::read_to_string(SETTINGS_PATH).await {
        Ok(value) => value,
        Err(error) => {
            log::error!(
                "Failed to read settings due to error: {}\nUsing default settings",
                error
            );
            return Settings::default();
        }
    };
    match toml::from_str(&content) {
        Ok(value) => value,
        Err(error) => {
            log::error!(
                "Failed to parse settings due to error: {}\nUsing default settings",
                error
            );
            return Settings::default();
        }
    }
}
pub async fn save_settings(settings: Settings) {
    RESTART_PENDING.store(true, std::sync::atomic::Ordering::Relaxed); // Set restart pending to true so they know to restart the system
    let content = toml::to_string(&settings).unwrap(); // This should never fail
    if let Err(error) = fs::write(SETTINGS_PATH, content).await {
        log::error!("Failed to save settings due to error: {}", error);
    }
}
