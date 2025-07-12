use alloy::primitives::{Address, U256};

/// Error type for market processing.
#[derive(Debug, Copy, Clone, thiserror::Error, PartialEq, Eq)]
pub enum MarketError {
    /// Insufficient fill to settle a trade.
    #[error("Insufficient fill when taking from context. Expected {amount} of {asset} from {recipient} on chain {chain_id}")]
    InsufficientFill {
        /// The chain ID on which the asset is deployed.
        chain_id: u64,
        /// The asset we expected to be in the context.
        asset: Address,
        /// The recipient account for which we tried to decrease the fill
        /// amount.
        recipient: Address,
        /// The amount by which we tried to decrease the fill.
        amount: U256,
    },
    /// Missing asset in the context.
    #[error("No fills of asset when taking from context. Expected {asset} on chain {chain_id}")]
    MissingAsset {
        /// The chain ID on which the asset is deployed.
        chain_id: u64,
        /// The asset we expected to be in the context.
        asset: Address,
    },
}
