use std::{borrow::Cow, str::FromStr};

/// Signet Environment constants.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct SignetEnvironmentConstants {
    /// Name of the host chain.
    host_name: Cow<'static, str>,
    /// Name of the rollup.
    rollup_name: Cow<'static, str>,
    /// URL of the Transaction Cache
    transaction_cache: Cow<'static, str>,
}

impl SignetEnvironmentConstants {
    /// Create a new set of environment constants.
    pub const fn new(
        host_name: Cow<'static, str>,
        rollup_name: Cow<'static, str>,
        transaction_cache: Cow<'static, str>,
    ) -> Self {
        Self { host_name, rollup_name, transaction_cache }
    }

    /// Get the hard-coded pecorino rollup constants.
    pub const fn pecorino() -> Self {
        crate::chains::pecorino::PECORINO_ENV
    }

    /// Get the hard-coded local test rollup constants.
    #[cfg(any(test, feature = "test-utils"))]
    pub const fn test() -> Self {
        crate::chains::test_utils::TEST_ENV
    }

    /// Get the host name.
    pub fn host_name(&self) -> &str {
        self.host_name.as_ref()
    }

    /// Get the rollup name.
    pub fn rollup_name(&self) -> &str {
        self.rollup_name.as_ref()
    }

    /// Get the transaction cache URL.
    pub fn transaction_cache(&self) -> &str {
        self.transaction_cache.as_ref()
    }
}

/// Error type for parsing struct from a chain name.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseChainError {
    /// The chain name is not supported.
    #[error("chain name {0} is not parseable. supported chains: {1}")]
    ChainNotSupported(String, String),
}

impl FromStr for SignetEnvironmentConstants {
    type Err = ParseChainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_lowercase();
        match s.as_str() {
            "pecorino" => Ok(Self::pecorino()),
            #[cfg(any(test, feature = "test-utils"))]
            "test" => Ok(Self::test()),
            _ => Err(ParseChainError::ChainNotSupported(s, "pecorino, test".to_string())),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn load_built_ins() {
        // deserialize json

        let json = serde_json::json!({
            "host_name": "pecorino",
            "rollup_name": "pecorino",
            "transaction_cache": "https://pecorino.com"
        });

        let s = serde_json::from_value::<SignetEnvironmentConstants>(json.clone()).unwrap();
        assert_eq!(serde_json::to_value(&s).unwrap(), json)
    }
}
