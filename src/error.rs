use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Feed parsing failed for {url}: {msg}")]
    FeedParse { url: String, msg: String },

    #[error("Transcription failed: {0}")]
    Transcription(String),

    #[error("Claude API error ({status}): {body}")]
    ClaudeApi { status: u16, body: String },

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, AppError>;
