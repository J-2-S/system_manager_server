mod secure;
use crate::handlers::shell::start_shell;
use crate::handlers::status::get_status;
use crate::templates::HomeTemplate;
use axum::Form;
use axum::http::Request;
use axum::http::StatusCode;
use axum::response::Result as AxumResult;
use axum::response::{Html, Redirect};
use axum::routing::post;
use axum::{Router, extract::WebSocketUpgrade, response::IntoResponse, routing::get};
use secure::{CertError, load_ssl_config};
use serde::Deserialize;
use std::{fmt, net::ToSocketAddrs, path::Path, sync::Arc};
use system_manager_server::auth;
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
        .route("/ws/{*wildcard}", get(ws_handler))
        .route("/", get(index))
        .route("/login", post(login))
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
async fn ws_handler<T>(ws: WebSocketUpgrade, req: Request<T>) -> impl IntoResponse {
    ws.on_upgrade(|socket| async move { todo!() })
}
