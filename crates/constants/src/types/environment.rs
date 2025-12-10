use crate::{KnownChains, ParseChainError};
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

    /// Get the hard-coded Mainnet environment constants.
    pub const fn mainnet() -> Self {
        crate::chains::mainnet::MAINNET_ENV
    }

    /// Get the hard-coded Parmigiana environment constants.
    pub const fn parmigiana() -> Self {
        crate::chains::parmigiana::PARMIGIANA_ENV
    }

    /// Get the hard-coded Pecorino environment constants.
    #[deprecated(note = "Pecorino is being deprecated in favor of Parmigiana")]
    #[allow(deprecated)]
    pub const fn pecorino() -> Self {
        crate::chains::pecorino::PECORINO_ENV
    }

    /// Get the hard-coded local test rollup constants.
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

impl TryFrom<KnownChains> for SignetEnvironmentConstants {
    type Error = ParseChainError;

    fn try_from(chain: KnownChains) -> Result<Self, Self::Error> {
        match chain {
            KnownChains::Mainnet => Ok(Self::mainnet()),
            KnownChains::Parmigiana => Ok(Self::parmigiana()),
            #[allow(deprecated)]
            KnownChains::Pecorino => Ok(Self::pecorino()),
            KnownChains::Test => Ok(Self::test()),
        }
    }
}

impl FromStr for SignetEnvironmentConstants {
    type Err = ParseChainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<KnownChains>()?.try_into()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn load_built_ins() {
        // deserialize json

        let json = serde_json::json!({
            "host_name": "parmigiana",
            "rollup_name": "parmigiana",
            "transaction_cache": "https://parmigiana.com"
        });

        let s = serde_json::from_value::<SignetEnvironmentConstants>(json.clone()).unwrap();
        assert_eq!(serde_json::to_value(&s).unwrap(), json)
    }
}
