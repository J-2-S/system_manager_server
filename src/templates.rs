use askama::Template;
#[derive(Template)]
#[template(path = "home.html")]
pub struct HomeTemplate<'a> {
    pub username: &'a str,
    pub low_power: bool,
    pub low_storage: bool,
    pub updates_available: bool,
    //add more as needed
}
