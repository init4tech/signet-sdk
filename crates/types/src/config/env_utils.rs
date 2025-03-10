use crate::ConfigError;
use alloy::primitives::Address;
use std::{borrow::Cow, env};

/// Load a variable from the environment
pub fn load_string(key: &str) -> Result<String, ConfigError> {
    env::var(key).map_err(|_| ConfigError::missing(key))
}

/// Load a variable from the environment
pub fn load_string_opt(key: &str) -> Option<String> {
    env::var(key).ok()
}

/// Load a variable from the environment
pub fn load_u64(key: &str) -> Result<u64, ConfigError> {
    let val = load_string(key)?;
    val.parse::<u64>().map_err(Into::into)
}

/// Load a variable from the environment
pub fn load_url(key: &str) -> Result<Cow<'static, str>, ConfigError> {
    load_string(key).map(Into::into)
}

/// Load a variable from the environment
pub fn load_url_opt(key: &str) -> Option<Cow<'static, str>> {
    load_string_opt(key).map(Into::into)
}

/// Load a variable from the environment
pub fn load_address(key: &str) -> Result<Address, ConfigError> {
    load_string(key)?.parse().map_err(Into::into)
}

/// Load a variable from the environment
pub fn load_u16_opt(key: &str) -> Option<u16> {
    load_string_opt(key)?.parse().ok()
}
