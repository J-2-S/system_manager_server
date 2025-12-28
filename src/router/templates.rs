use askama::Template;
#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub error: bool,
}
#[derive(Template)]
#[template(path = "home.html")]
pub struct HomeTemplate {
    pub username: String,
    pub low_power: bool,
    pub low_storage: bool,
    pub updates_available: bool,
    pub restart_pending: bool,
    pub is_admin: bool,
}
#[derive(Template)]
#[template(path = "settings.html")]
pub struct SettingsTemplate {
    pub low_storage: u8,
    pub low_power: u8,
    pub ignore_update: bool,
    pub cert_path: String,
    pub key_path: String,
    pub port: u16,
    pub hostname: String,
}
#[derive(Template)]
#[template(path = "management.html")]
pub struct ManageTemplate {
    pub users: Vec<String>,
}
#[derive(Template)]
#[template(path = "user_settings.html")]
pub struct UserSettingsTemplate {
    pub storage: usize,
    pub admin: bool,
    pub user: String,
}
