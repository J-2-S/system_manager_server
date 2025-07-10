mod secure;
use crate::handlers::shell::start_shell;
use crate::handlers::status::get_status;
use crate::settings;
use crate::templates::HomeTemplate;
use crate::templates::ManagementTemplate;
use crate::templates::SettingsTemplate;
use askama::Template;
use axum::Form;
use axum::http::StatusCode;
use axum::response::Result as AxumResult;
use axum::response::{Html, Redirect};
use axum::routing::post;
use axum::{Router, extract::WebSocketUpgrade, response::IntoResponse, routing::get};
use secure::{CertError, load_ssl_config};
use serde::Deserialize;
use std::{fmt, net::ToSocketAddrs, path::Path, sync::Arc};
use system_manager_server::auth;
use system_manager_server::auth::is_group_leader;
use system_manager_server::auth::is_sudo;
use tokio::net::TcpListener;
use tokio::task;
use tokio_rustls::TlsAcceptor;
use tower_http::services;
use tower_sessions::{MemoryStore, Session, SessionManagerLayer};

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    username: String,
    password: String,
}

#[derive(Debug)]
pub enum ServerError {
    CertError(CertError),
    IoError(std::io::Error),
}
impl From<CertError> for ServerError {
    fn from(value: CertError) -> Self {
        Self::CertError(value)
    }
}
impl From<std::io::Error> for ServerError {
    fn from(value: std::io::Error) -> Self {
        Self::IoError(value)
    }
}
impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CertError(error) => write!(f, "{}", error),
            Self::IoError(error) => write!(f, "{}", error),
        }
    }
}

pub async fn start<A>(key_path: &Path, cert_path: &Path, addrs: A) -> Result<(), ServerError>
where
    A: ToSocketAddrs + tokio::net::ToSocketAddrs,
{
    let config = Arc::new(load_ssl_config(key_path, cert_path).await?);
    let acceptor = TlsAcceptor::from(config.clone());
    let listener = TcpListener::bind(addrs).await?;
    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false) // Make this true to enable HTTPS
        .with_expiry(tower_sessions::Expiry::OnInactivity(
            tower_sessions::cookie::time::Duration::seconds(15 * 60),
        )); //There is a 15 minute expiry

    let app = Router::new()
        .route("/ws/shell", get(shell_handler))
        .route("/settings", get(settings_handler))
        .route("/settings", post(settings_save))
        .route("/", get(index))
        .route("/login", post(login))
        .route("/manage", get(manage_handler))
        .route(
            "/login",
            get(async || Redirect::to("/static/login.html").into_response()),
        )
        .route("/logout", get(logout))
        .nest_service("/static/", services::ServeDir::new("static"))
        .nest_service("/node_modules/", services::ServeDir::new("node_modules"))
        .route_layer(session_layer);
    let service = app.into_make_service();
    axum::serve(listener, service).await?;
    Ok(())
}

async fn index(session: Session) -> AxumResult<impl IntoResponse> {
    let username: String = session
        .get("username")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .unwrap_or_default();

    let password: String = session
        .get("password")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .unwrap_or_default();
    if username.is_empty() || password.is_empty() {
        return Ok(Redirect::to("/static/login.html").into_response());
    }
    println!("Auth user: {username}");

    let current_user = match auth::auth_user(&username, &password) {
        Ok(user) => user,
        Err(_) => {
            return Ok(Redirect::to("/static/login.html").into_response());
        }
    };

    let name = current_user.name().to_string_lossy().into_owned();

    let status = task::spawn_blocking(get_status)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let html = HomeTemplate {
        username: &name,
        low_storage: status.low_storage,
        low_power: status.low_power,
        updates_available: !status.up_to_date,
    };

    Ok(Html(html.to_string()).into_response())
}

#[derive(Debug, Deserialize)]
struct SettingsForm {
    low_storage: u8,
    low_power: u8,
    ignore_update: bool,
    key_path: String,
    cert_path: String,
}
async fn settings_save(
    session: Session,
    Form(settings): Form<SettingsForm>,
) -> AxumResult<impl IntoResponse> {
    let username: String = session
        .get("username")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .unwrap_or_default();
    let password: String = session
        .get("password")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .unwrap_or_default();
    if username.is_empty() || password.is_empty() {
        Ok(Redirect::to("/static/login.html").into_response())
    } else if let Ok(user) = auth::auth_user(&username, &password) {
        if !is_sudo(&user) {
            return Ok(Redirect::to("/").into_response());
        }
        let mut sets = settings::load_settings().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        sets.thresholds.low_storage = settings.low_storage;
        sets.thresholds.low_power = settings.low_power;
        sets.ignore_update = settings.ignore_update;
        sets.paths.key_path = settings.key_path;
        sets.paths.cert_path = settings.cert_path;
        settings::save_settings(&sets).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Ok(Redirect::to("/settings").into_response())
    } else {
        Ok(Redirect::to("/static/login.html").into_response())
    }
}
async fn login(session: Session, Form(login): Form<LoginForm>) -> AxumResult<impl IntoResponse> {
    let username = login.username;
    let password = login.password;

    println!("Login attempt: {username} {password}");

    session
        .insert("username", &username)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    session
        .insert("password", &password)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Redirect::to("/"))
}

async fn logout(session: Session) -> AxumResult<impl IntoResponse> {
    session
        .delete()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Redirect::to("/login").into_response())
}
async fn shell_handler(ws: WebSocketUpgrade, session: Session) -> AxumResult<impl IntoResponse> {
    let username: String = session
        .get("username")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .unwrap_or_default();
    let password: String = session
        .get("password")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .unwrap_or_default();
    if username.is_empty() || password.is_empty() {
        Ok(Redirect::to("/static/login.html").into_response())
    } else {
        if let Ok(user) = auth::auth_user(&username, &password) {
            Ok(ws
                .on_upgrade(|socket| async move {
                    task::spawn_blocking(move || {
                        let _ = start_shell(socket, user)
                            .map_err(|e| eprintln!("Failed to start shell: {e}"));
                    })
                    .await
                    .unwrap();
                })
                .into_response())
        } else {
            Ok(Redirect::to("/static/login.html").into_response())
        }
    }
}
async fn manage_handler(session: Session) -> AxumResult<impl IntoResponse> {
    let username: String = session
        .get("username")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .unwrap_or_default();
    let password: String = session
        .get("password")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .unwrap_or_default();
    if username.is_empty() || password.is_empty() {
        Ok(Redirect::to("/static/login.html").into_response())
    } else if let Ok(user) = auth::auth_user(&username, &password) {
        if is_sudo(&user) {
            let mut groups = sysinfo::Groups::new_with_refreshed_list()
                .iter()
                .map(|g| g.name().to_owned())
                .collect::<Vec<String>>();
            groups.push("all".into());
            let html = ManagementTemplate { groups };
            Ok(Html(html.render().unwrap()).into_response())
        } else {
            let groups = user.groups().unwrap_or_default();
            let mut names = Vec::new();
            for group in groups {
                if is_group_leader(&user, &group).await {
                    names.push(group.name().to_owned().to_string_lossy().into_owned());
                }
            }
            let html = ManagementTemplate { groups: names };
            Ok(Html(html.render().unwrap()).into_response())
        }
    } else {
        Ok(Redirect::to("/static/login.html").into_response())
    }
}
async fn settings_handler(session: Session) -> AxumResult<impl IntoResponse> {
    let username: String = session
        .get("username")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .unwrap_or_default();
    let password: String = session
        .get("password")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .unwrap_or_default();
    if username.is_empty() || password.is_empty() {
        Ok(Redirect::to("/static/login.html").into_response())
    } else if let Ok(user) = auth::auth_user(&username, &password) {
        if !is_sudo(&user) {
            return Ok(Redirect::to("/").into_response());
        }
        let sets = settings::load_settings().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let html: SettingsTemplate = sets.into();
        Ok(Html(html.render().unwrap()).into_response())
    } else {
        Ok(Redirect::to("/static/login.html").into_response())
    }
}
