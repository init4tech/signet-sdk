use std::str::FromStr;

/// The list of known chains as a string.
const KNOWN_CHAINS: &str = "pecorino, test";

/// Error type for parsing struct from a chain name.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseChainError {
    /// The chain name is not supported.
    #[error("chain name {0} is not parseable. supported chains: {KNOWN_CHAINS}")]
    ChainNotSupported(String),
}

/// Known chains for the Signet system.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum KnownChains {
    /// Pecorino chain.
    Pecorino,
    /// Test chain.
    #[cfg(any(test, feature = "test-utils"))]
    Test,
}

impl FromStr for KnownChains {
    type Err = ParseChainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_lowercase();
        match s.as_str() {
            #[cfg(any(test, feature = "test-utils"))]
            "test" => Ok(Self::Test),
            "pecorino" => Ok(Self::Pecorino),
            _ => Err(ParseChainError::ChainNotSupported(s)),
        }
    }
}
