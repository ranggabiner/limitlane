use thiserror::Error;

#[derive(Error, Debug)]
pub enum LimitLaneError {
    #[error("Authentication error on provider '{provider}' for account '{account_id}': {message}")]
    Authentication { provider: String, account_id: String, message: String },

    #[error("Credential store error: {0}")]
    CredentialStore(String),

    #[error("Network error accessing '{url}': {message}")]
    Network { url: String, message: String },

    #[error("Provider '{provider}' error: {message}")]
    Provider { provider: String, message: String },

    #[error("Failed to parse provider response from '{provider}': {message}")]
    Parsing { provider: String, message: String },

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
