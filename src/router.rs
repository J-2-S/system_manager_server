use std::{collections::HashMap, sync::LazyLock};

use askama::Template;
use axum::{
    Form, Router,
    extract::Path,
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use serde::Deserialize;
use tokio::{net::TcpListener, task};
use tower_http::services::ServeDir;
use tower_sessions::{MemoryStore, Session, SessionManagerLayer, session::Id};

use crate::{
    RESTART_PENDING,
    router::templates::*,
    settings::{self, Settings},
    status,
    users::{self, UserError},
};
pub mod templates;

macro_rules! render {
    ($template:expr) => {
        match $template.render() {
            Ok(value) => value,
            Err(error) => {
                log::error!("Failed to render template: {}", error);
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to render page")
                    .into_response();
            }
        }
    };
}
macro_rules! tokio_blocking {
    ($task:expr) => {
        match tokio::task::spawn_blocking($task).await {
            Ok(value) => value,
            Err(error) => {
                log::error!("Failed to handle request due to runtime error: {}", &error);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to handle request due to runtime error",
                )
                    .into_response();
            }
        }
    };
}
macro_rules! tokio_async {
    ($task:expr) => {
        match task::spawn($task).await {
            Ok(value) => value,
            Err(error) => {
                log::error!("Failed to handle request due to runtime error: {}", &error);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to handle request due to runtime error",
                )
                    .into_response();
            }
        }
    };
}
macro_rules! err_response {
    ($error:expr) => {
        match $error {
            Ok(value) => value,
            Err(error) => {
                log::error!("Failed to handle request due to error: {}", &error);
                return error.into_response();
            }
        }
    };
}
macro_rules! get_current_user {
    ($session:expr) => {
        if let Some(username) = $session.get::<String>("username").await.unwrap_or_default() {
            err_response!(users::User::fetch_user(&username).await)
        } else {
            return Redirect::to("/").into_response();
        }
    };
}
static mut SESSION_USER: LazyLock<HashMap<String, Id>> = LazyLock::new(|| HashMap::new());
pub async fn init_router() {
    let mut user = users::User::new("linuxman", "!!Oct06Yes").await.unwrap();
    *user.admin_mut() = true;
    user.save().await.unwrap();
    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store).with_secure(false); // Set to true in production with HTTPS

    let listener = match TcpListener::bind("0.0.0.0:8080").await {
        Ok(value) => value,
        Err(error) => {
            log::error!("Failed to bind to port 80: {}", error);
            std::process::exit(1);
        }
    };

    let router = Router::new()
        .route("/", get(index))
        .route("/login", post(login))
        .route("/home", get(home))
        .route("/logout", get(logout))
        .route("/settings", get(get_settings))
        .route("/settings", post(post_settings))
        .route("/manage", get(management))
        .route("/manage/{user}", get(get_user_settings))
        .route("/manage/{user}", post(post_user_settings))
        .nest_service("/static/", ServeDir::new("static"))
        .layer(session_layer);

    if let Err(error) = axum::serve(listener, router).await {
        log::error!("Failed to start server: {}", error);
        std::process::exit(1);
    }
}

async fn index(session: Session) -> impl IntoResponse {
    if session
        .get::<String>("username")
        .await
        .unwrap_or_default()
        .is_some()
    {
        Redirect::to("/home").into_response()
    } else {
        let template = IndexTemplate { error: false };
        Html(render!(template)).into_response()
    }
}

async fn logout(session: Session) -> impl IntoResponse {
    if let Err(error) = session.delete().await {
        log::error!("Failed to delete session: {}", error);
    }
    Redirect::to("/").into_response()
}

#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

async fn login(session: Session, Form(form): Form<LoginForm>) -> impl IntoResponse {
    let username = form.username.trim();
    let password = form.password.trim();
    let user = users::User::login(username, password).await;
    match user {
        Ok(_) => {
            // Store only the username
            session
                .insert("username", username.to_string())
                .await
                .unwrap();
            Redirect::to("/home").into_response()
        }
        Err(error) => {
            if let UserError::PasswordError | UserError::UserNotFound = error {
                let template = IndexTemplate { error: true };
                Html(render!(template)).into_response()
            } else {
                error.into_response()
            }
        }
    }
}

async fn home(session: Session) -> impl IntoResponse {
    let user = get_current_user!(session);

    let thresholds = settings::load_settings().await.threatsholds;
    let low_power = match tokio_blocking!(status::check_power) {
        Ok(value) => value < thresholds.low_power,
        Err(error) => {
            log::error!("Failed to get low power: {}", error);
            false
        }
    };
    let low_storage = status::check_storage() < thresholds.low_storage;
    let template = HomeTemplate {
        username: user.name().to_string(),
        low_power,
        low_storage,
        updates_available: false,
        restart_pending: RESTART_PENDING.load(std::sync::atomic::Ordering::Relaxed),
        is_admin: user.admin(),
    };
    Html(render!(template)).into_response()
}

async fn get_settings(session: Session) -> impl IntoResponse {
    let user = get_current_user!(session);

    if !user.admin() {
        return (
            StatusCode::FORBIDDEN,
            "Forbidden only system admins can access this page",
        )
            .into_response();
    }

    let settings = settings::load_settings().await;
    let template: SettingsTemplate = settings.into();
    Html(render!(template)).into_response()
}

#[derive(Deserialize)]
struct SettingsForm {
    low_storage: u8,
    low_power: u8,
    #[serde(default)]
    ignore_update: bool,
    cert_path: String,
    key_path: String,
    hostname: String,
    port: u16,
}

impl Into<Settings> for SettingsForm {
    fn into(self) -> Settings {
        Settings {
            port: self.port,
            cert_path: self.cert_path.into(),
            key_path: self.key_path.into(),
            hostname: self.hostname.into(),
            ignore_updates: self.ignore_update,
            threatsholds: settings::Threasholds {
                low_power: self.low_power,
                low_storage: self.low_storage,
            },
        }
    }
}

async fn post_settings(session: Session, Form(form): Form<SettingsForm>) -> impl IntoResponse {
    let user = get_current_user!(session);

    if !user.admin() {
        return (
            StatusCode::FORBIDDEN,
            "Forbidden only system admins can access this page",
        )
            .into_response();
    }

    settings::save_settings(form.into()).await;
    Redirect::to("/settings").into_response()
}

async fn management(session: Session) -> impl IntoResponse {
    let user = get_current_user!(session);

    if !user.admin() {
        return (
            StatusCode::FORBIDDEN,
            "Forbidden only system admins can access this page",
        )
            .into_response();
    }

    let users = err_response!(users::get_users().await)
        .iter()
        .map(|value| value.name().to_string())
        .collect();
    let template = ManageTemplate { users };
    Html(render!(template)).into_response()
}

async fn get_user_settings(session: Session, Path(username): Path<String>) -> impl IntoResponse {
    let user = get_current_user!(session);

    if !user.admin() {
        return (
            StatusCode::FORBIDDEN,
            "Forbidden only system admins can access this page",
        )
            .into_response();
    }

    // utu stands for user to update
    let utu = err_response!(users::User::fetch_user(&username).await);
    let template = UserSettingsTemplate {
        storage: utu.storage(),
        admin: utu.admin(),
        user: username.to_string(),
    };
    Html(render!(template)).into_response()
}

#[derive(Deserialize)]
struct UserSettingsForm {
    #[serde(default)]
    admin: bool,
    storage: usize,
    password: String,
}

async fn post_user_settings(
    session: Session,
    Path(username): Path<String>,
    Form(form): Form<UserSettingsForm>,
) -> impl IntoResponse {
    let user = get_current_user!(session);

    if !user.admin() {
        return (
            StatusCode::FORBIDDEN,
            "Forbidden only system admins can access this page",
        )
            .into_response();
    }

    // utu stands for user to update
    let mut utu = err_response!(users::User::fetch_user(&username).await);
    log::info!("admin: {}", form.admin);
    *utu.admin_mut() = form.admin;
    *utu.storage_mut() = form.storage;
    if !form.password.trim().is_empty() {
        let password = form.password.trim().to_string();
        err_response!(utu.change_password(password).await);
    } else {
        err_response!(utu.save().await);
    }
    Redirect::to("/manage").into_response()
}
