/// Signet Environment constants.
#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct SignetEnvironmentConstants {
    /// Name of the host chain.
    host_name: &'static str,
    /// Name of the rollup.
    rollup_name: &'static str,
    /// URL of the Transaction Cache
    transaction_cache: &'static str,
}

impl SignetEnvironmentConstants {
    /// Create a new set of environment constants.
    pub const fn new(
        host_name: &'static str,
        rollup_name: &'static str,
        transaction_cache: &'static str,
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
    pub const fn host_name(&self) -> &str {
        self.host_name
    }

    /// Get the rollup name.
    pub const fn rollup_name(&self) -> &str {
        self.rollup_name
    }

    /// Get the transaction cache URL.
    pub const fn transaction_cache(&self) -> &str {
        self.transaction_cache
    }

    /// Get the transaction cache URL.
    pub fn transaction_cache_url(&self) -> reqwest::Url {
        reqwest::Url::parse(self.transaction_cache).expect("Invalid transaction cache URL")
    }
}
