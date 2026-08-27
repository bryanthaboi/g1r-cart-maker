//! One error type across the command layer: a short message for the user and a
//! detail string for the expandable log.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AppError {
    pub message: String,
    pub detail: String,
    pub kind: &'static str,
}

impl AppError {
    pub fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            detail: String::new(),
            kind,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    pub fn io(message: impl Into<String>, problem: std::io::Error) -> Self {
        Self::new("io", message).with_detail(problem.to_string())
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid", message)
    }

    pub fn network(message: impl Into<String>) -> Self {
        Self::new("network", message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new("not_found", message)
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "{}", self.message)
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(problem: std::io::Error) -> Self {
        Self::new("io", problem.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(problem: serde_json::Error) -> Self {
        Self::new("invalid", problem.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
