pub mod status;
pub mod users;
use std::error::Error;
#[derive(Debug, Clone, PartialEq)]
pub enum HandleError {
    HandlerFailed(String),
    HandlerNotFound(String),
    InvaildUser(String),
}
impl Error for HandleError {}

impl std::fmt::Display for HandleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HandlerFailed(error) => write!(f, "Handler failed: {}", error),
            Self::HandlerNotFound(error) => write!(f, "Handler not found: {}", error),
            Self::InvaildUser(error) => write!(f, "Invalid user: {}", error),
        }
    }
}

impl From<String> for HandleError {
    fn from(value: String) -> Self {
        Self::HandlerFailed(value)
    }
}
impl From<&str> for HandleError {
    fn from(value: &str) -> Self {
        Self::HandlerFailed(value.to_string())
    }
}

impl From<Box<dyn std::error::Error>> for HandleError {
    fn from(value: Box<dyn std::error::Error>) -> Self {
        Self::HandlerFailed(value.to_string())
    }
}
