use std::fs::File;
use std::io::{self, Read, Write};
use serde::{Serialize, Deserialize};
use std::error::Error;
const SETTINGS_FILE: &str = "./settings.json";
#[derive(Debug)]
pub enum SettingError {
    IOError(io::Error),
    JsonError(serde_json::Error)
}
impl Error for SettingError {}
impl std::fmt::Display for SettingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self{
            Self::IOError(error)=> write!(f,"{}",error),
            Self::JsonError(error)=> write!(f,"{}",error)
        }
    }
}
impl From<io::Error> for SettingError {
    fn from(value: io::Error) -> Self {
        Self::IOError(value)
    }
}
impl From<serde_json::Error> for SettingError {
    fn from(value: serde_json::Error) -> Self {
        Self::JsonError(value)
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct Thresholds {
    pub low_storage: u64,
    pub low_power: f32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Paths {
    pub module_path: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Settings {
    pub thresholds: Thresholds,
    pub paths : Paths,
}

pub fn save_settings(settings: &Settings) -> Result<(), SettingError> {
    let encoded = serde_json::to_string_pretty(settings)?;
    let mut file = File::create(SETTINGS_FILE)?;
    file.write_all(encoded.as_bytes())?;
    Ok(())
}

pub fn load_settings() -> Result<Settings, SettingError> {
    let mut file = File::open(SETTINGS_FILE)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let settings = serde_json::from_str(&contents)?;
    Ok(settings)
}

pub fn get_or_create_settings() -> Result<Settings, SettingError> {
    match load_settings() {
        Ok(settings) => Ok(settings),
        Err(_) => {
            // If settings file is missing or invalid, create default settings.
            let default_settings = Settings {
                thresholds: Thresholds {
                    low_storage: 1_000,
                    low_power: 2.0,
                },
                paths : Paths {
                    module_path: String::from("./plugins"),
                },
            };
            save_settings(&default_settings)?;
            Ok(default_settings)
        }
    }
}

