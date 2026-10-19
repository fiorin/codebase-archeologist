use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid repository: {0}")]
    InvalidRepository(String),

    #[error("job cancelled")]
    Cancelled,

    #[error("parse error in {path}: {message}")]
    Parse { path: String, message: String },

    #[error("{0}")]
    Other(String),
}

impl CoreError {
    pub fn other(msg: impl Into<String>) -> Self {
        CoreError::Other(msg.into())
    }
}

pub type Result<T> = std::result::Result<T, CoreError>;
