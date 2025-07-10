use askama::Template;

use crate::settings::Settings;
#[derive(Template)]
#[template(path = "home.html")]
pub struct HomeTemplate<'a> {
    pub username: &'a str,
    pub low_power: bool,
    pub low_storage: bool,
    pub updates_available: bool,
    //add more as needed
}
#[derive(Template)]
#[template(path = "settings.html")]
pub struct SettingsTemplate {
    pub low_storage: u8,
    pub low_power: u8,
    pub ignore_update: bool,
    pub key_path: String,
    pub cert_path: String,
}
impl From<Settings> for SettingsTemplate {
    fn from(value: Settings) -> Self {
        Self {
            low_storage: value.thresholds.low_storage,
            low_power: value.thresholds.low_power,
            ignore_update: value.ignore_update,
            key_path: value.paths.key_path,
            cert_path: value.paths.cert_path,
        }
    }
}
#[derive(Template)]
#[template(path = "management.html")]
pub struct ManagementTemplate {
    pub groups: Vec<String>,
}
