use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Metadata error: {0}")]
    Metadata(String),
    #[error("Playback error: {0}")]
    Playback(String),
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Provider error ({provider}): {message}")]
    Provider { provider: String, message: String },
    #[error("{0}")]
    General(String),
}

impl From<AppError> for String {
    fn from(e: AppError) -> String {
        e.to_string()
    }
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
