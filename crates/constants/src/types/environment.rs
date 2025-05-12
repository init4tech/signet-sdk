/// Signet Environment constants.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct SignetEnvironmentConstants {
    /// Name of the host chain.
    host_name: String,
    /// Name of the rollup.
    rollup_name: String,
    /// URL of the Transaction Cache
    transaction_cache: String,
}

impl SignetEnvironmentConstants {
    /// Create a new set of environment constants.
    pub const fn new(host_name: String, rollup_name: String, transaction_cache: String) -> Self {
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
        &self.host_name
    }

    /// Get the rollup name.
    pub fn rollup_name(&self) -> &str {
        &self.rollup_name
    }

    /// Get the transaction cache URL.
    pub fn transaction_cache(&self) -> &str {
        &self.transaction_cache
    }
}
