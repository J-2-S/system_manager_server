use std::{
    collections::HashMap,
    path::PathBuf,
    str::FromStr,
    sync::{LazyLock, Once},
};

use argon2::{
    Argon2, PasswordVerifier,
    password_hash::{
        self, PasswordHashString, PasswordHasher, SaltString,
        rand_core::{OsRng, RngCore},
    },
};
use axum::{http::StatusCode, response::IntoResponse};
use base64::Engine;
use serde::{Deserialize, Serialize};
use tokio::{fs, sync::RwLock, task};
#[cfg(not(debug_assertions))]
#[cfg(target_os = "linux")]
const USERS_PATH: &str = "/var/lib/system_manager_server/users";

#[cfg(not(debug_assertions))]
#[cfg(not(target_os = "linux"))]
compile_error!("Only Linux is supported right now");

#[cfg(debug_assertions)]
const USERS_PATH: &str = "./users";
static PASSWORD_HASHER: LazyLock<Argon2> = LazyLock::new(|| Argon2::default());
#[derive(Debug)]
pub enum UserError {
    IoError(std::io::Error),
    PasswordError,
    UserNotFound,
    UserExists,
    Other(String),
}
impl From<std::io::Error> for UserError {
    fn from(value: std::io::Error) -> Self {
        log::error!("IO Error: {}", value);
        UserError::IoError(value)
    }
}
impl From<serde_json::Error> for UserError {
    fn from(value: serde_json::Error) -> Self {
        log::error!("Postcard Error: {}", value);
        UserError::Other(value.to_string())
    }
}
impl std::fmt::Display for UserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(error) => write!(f, "IO Error: {error}"),
            Self::PasswordError => write!(f, "Password Error"),
            Self::UserNotFound => write!(f, "User Not Found"),
            Self::UserExists => write!(f, "User Exists"),
            Self::Other(error) => write!(f, "{}", error),
        }
    }
}
impl std::error::Error for UserError {}
impl IntoResponse for UserError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::IoError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "IO Error").into_response(),
            Self::PasswordError => (StatusCode::BAD_REQUEST, "Password Error").into_response(),
            Self::UserNotFound => (StatusCode::NOT_FOUND, "User Not Found").into_response(),
            Self::UserExists => (StatusCode::CONFLICT, "User Exists").into_response(),
            Self::Other(error) => (StatusCode::INTERNAL_SERVER_ERROR, error).into_response(),
        }
    }
}
type Result<T> = std::result::Result<T, UserError>;
macro_rules! tokio_error {
    ($e:expr) => {
        match $e {
            Ok(value) => value,
            Err(error) => {
                log::error!("Failed to spawn blocking task: {}", error);
                return Err(UserError::Other(
                    "Failed to spawn blocking task".to_string(),
                ));
            }
        }
    };
}
/// A list of the usernames of all users that have logged in
static LOGGED_IN_USERS: LazyLock<RwLock<Vec<String>>> = LazyLock::new(|| RwLock::new(Vec::new()));
static ENSURE_DIR: Once = Once::new();
/// A user on within system manager server (not necessarily a system user just a user in our database)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    name: String,
    password: String,
    admin: bool,
    storage: usize, // the amount of storage the user has in byte
}
impl User {
    /// Creates a new user from a name and password
    pub async fn new(name: &str, password: &str) -> Result<Self> {
        ENSURE_DIR.call_once(|| {
            std::fs::create_dir_all(USERS_PATH).unwrap();
        });
        let password = password.trim().to_string();
        let password_hash = tokio_error!(
            task::spawn_blocking(move || {
                let salt = SaltString::generate(&mut OsRng);
                let password_hash = match PASSWORD_HASHER.hash_password(password.as_bytes(), &salt)
                {
                    Ok(value) => value,
                    Err(error) => {
                        log::error!("Failed to hash password: {}", error);
                        return String::new();
                    }
                };
                password_hash.to_string()
            })
            .await
        );
        let user = Self {
            name: name.trim().to_string(),
            password: password_hash,
            admin: false,
            storage: 1073741824, // 1 GB is the default storage size
        };
        user.clone().save().await?; // save the user to the database so we can fetch it later as
        // needed
        Ok(user)
    }

    #[inline]
    pub fn name<'a>(&'a self) -> &'a str {
        &self.name
    }

    #[inline]
    pub fn admin(&self) -> bool {
        self.admin
    }
    #[inline]
    pub fn storage(&self) -> usize {
        self.storage
    }
    #[inline]
    pub fn name_mut<'a>(&'a mut self) -> &'a mut str {
        &mut self.name
    }
    #[inline]
    pub fn admin_mut(&mut self) -> &mut bool {
        &mut self.admin
    }
    #[inline]
    pub fn storage_mut(&mut self) -> &mut usize {
        &mut self.storage
    }

    pub async fn login(username: &str, password: &str) -> Result<Self> {
        let user_file = PathBuf::from(USERS_PATH).join(username).join("user.json");
        if fs::try_exists(&user_file).await.unwrap_or(false) {
            let data = fs::read(&user_file).await?;
            let user: User = serde_json::from_slice(&data)?;
            let vaild_password = {
                let hashed_password = user.password.clone();
                let password = password.to_string();
                tokio_error!(
                    task::spawn_blocking(move || {
                        // Okay the sytnax here cloud be cleaner but I'm too lazy to clean it up so
                        // I'll just explain it
                        match PASSWORD_HASHER.verify_password(
                            password.as_bytes(),
                            //we have to make sure it is borrowed here
                            &(match PasswordHashString::from_str(&hashed_password) {
                                // okay here we
                                // load the password hash from the string
                                Ok(value) => value, // then if it's okay we use the
                                // password hash string
                                Err(_) => return false,
                            }
                            .password_hash()), // then we convert the password hash string to a password hash (it's some weird typing thing)
                        ) {
                            Ok(_) => true,   // if the password is valid we return true
                            Err(_) => false, // if the password is invalid we return false
                        }
                    })
                    .await
                )
            };
            if !vaild_password {
                return Err(UserError::PasswordError);
            } else {
                Ok(user)
            }
        } else {
            Err(UserError::UserNotFound)
        }
    }

    pub async fn fetch_user(username: &str) -> Result<User> {
        let username = username.trim();
        let user_file = PathBuf::from(USERS_PATH).join(username).join("user.json");
        if fs::try_exists(&user_file).await.unwrap_or(false) {
            let data = fs::read(&user_file).await?;
            let user: User = serde_json::from_slice(&data)?;
            Ok(user)
        } else {
            Err(UserError::UserNotFound)
        }
    }

    pub async fn save(self) -> Result<()> {
        let username = self.name.trim();
        let user_dir = PathBuf::from(USERS_PATH).join(username);
        let user_file = user_dir.join("user.json");
        fs::create_dir_all(&user_dir).await?;
        let data = serde_json::to_string(&self)?;
        fs::write(&user_file, data.as_bytes()).await?;
        Ok(())
    }

    pub async fn delete(self) -> Result<()> {
        let username = self.name.trim();
        let user_dir = PathBuf::from(USERS_PATH).join(username);
        fs::remove_dir_all(user_dir).await?;
        Ok(())
    }
    pub async fn change_password(mut self, new_password: String) -> Result<String> {
        let hashed_password = {
            let new_password = new_password.clone();
            tokio_error!(
                task::spawn_blocking(move || {
                    let salt = SaltString::generate(&mut OsRng);
                    let password_hash =
                        match PASSWORD_HASHER.hash_password(new_password.as_bytes(), &salt) {
                            Ok(value) => value,
                            Err(error) => {
                                log::error!("Failed to hash password: {}", error);
                                return String::new();
                            }
                        };
                    password_hash.to_string()
                })
                .await
            )
        };
        self.password = hashed_password;
        self.save().await?;
        Ok(new_password)
    }
}

pub async fn get_users() -> Result<Vec<User>> {
    let user_dir = PathBuf::from(USERS_PATH);
    let mut read_dir = fs::read_dir(user_dir).await?;
    let mut users = Vec::new();
    while let Some(entry) = read_dir.next_entry().await? {
        let path = entry.path();
        if let Some(name) = path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
        {
            users.push(User::fetch_user(&name).await?);
        }
    }
    Ok(users)
}
