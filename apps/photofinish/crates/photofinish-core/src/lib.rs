pub mod db;
pub mod event;
pub mod hardware;
pub mod id;
pub mod imaging;
pub mod preset;
pub mod roster;
pub mod server;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(String),

    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Not a PhotoFinish event bundle: {0}")]
    NotAnEventBundle(String),

    #[error("Event schema v{db_version} is newer than this version of PhotoFinish (supports up to v{app_version}). Please update PhotoFinish before opening this event.")]
    SchemaTooNew { db_version: i64, app_version: i64 },

    #[error("Image processing error: {0}")]
    Image(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, Error>;
