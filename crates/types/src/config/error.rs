/// Error type for [`crate::config`] module. Captures errors related to loading
/// configuration from the environment or other sources.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Missing `signetConstants` genesis field.
    #[error("missing signetConstants field in genesis")]
    MissingGenesis(&'static str),
    /// Error loading from environment variable
    #[error("missing or non-unicode environment variable: {0}")]
    Var(String),
    /// Error parsing environment variable
    #[error("failed to parse environment variable: {0}")]
    Parse(#[from] std::num::ParseIntError),
    /// Error parsing boolean environment variable
    #[error("failed to parse boolean environment variable")]
    ParseBool,
    /// Error parsing hex from environment variable
    #[error("failed to parse hex: {0}")]
    Hex(#[from] hex::FromHexError),
    /// Error parsing JSON
    #[error("failed to parse JSON: {0}")]
    Json(#[from] serde_json::Error),
}

impl ConfigError {
    /// Missing or non-unicode env var.
    pub fn missing(s: &str) -> Self {
        ConfigError::Var(s.to_string())
    }
}
