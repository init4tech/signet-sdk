use alloy::primitives::{Address, U256};

/// Error type for market processing.
#[derive(Debug, Copy, Clone, thiserror::Error, PartialEq, Eq)]
pub enum MarketError {
    /// Insufficient balance to settle a trade.
    #[error("Insufficient balance when taking from context")]
    InsufficientBalance { chain_id: u64, asset: Address, recipient: Address, amount: U256 },
    /// Missing asset in the context.
    #[error("No recipients of asset when taking from context")]
    MissingAsset { chain_id: u64, asset: Address },
}
