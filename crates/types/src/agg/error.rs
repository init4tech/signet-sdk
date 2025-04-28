use alloy::primitives::{Address, U256};

/// Error type for market processing.
#[derive(Debug, Copy, Clone, thiserror::Error, PartialEq, Eq)]
pub enum MarketError {
    /// Insufficient balance to settle a trade.
    #[error("Insufficient balance when taking from context")]
    InsufficientBalance {
        /// The chain ID on which the asset is deployed.
        chain_id: u64,
        /// The asset we expected to be in the context.
        asset: Address,
        /// The recipient account we tried to take from.
        recipient: Address,
        /// The amount we tried to take.
        amount: U256,
    },
    /// Missing asset in the context.
    #[error("No recipients of asset when taking from context")]
    MissingAsset {
        /// The chain ID on which the asset is deployed.
        chain_id: u64,
        /// The asset we expected to be in the context.
        asset: Address,
    },
}
