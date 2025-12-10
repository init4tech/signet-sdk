use std::str::FromStr;

/// The list of known chains as a string.
const KNOWN_CHAINS: &str = "mainnet, parmigiana, pecorino, test";

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
    /// Mainnet chain.
    Mainnet,
    /// Parmigiana chain.
    Parmigiana,
    /// Pecorino chain.
    #[deprecated(note = "Pecorino is being deprecated in favor of Parmigiana")]
    Pecorino,
    /// Test chain.
    Test,
}

impl FromStr for KnownChains {
    type Err = ParseChainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_lowercase();
        match s.as_str() {
            "mainnet" => Ok(Self::Mainnet),
            "parmigiana" => Ok(Self::Parmigiana),
            #[allow(deprecated)]
            "pecorino" => Ok(Self::Pecorino),
            "test" => Ok(Self::Test),
            _ => Err(ParseChainError::ChainNotSupported(s)),
        }
    }
}
