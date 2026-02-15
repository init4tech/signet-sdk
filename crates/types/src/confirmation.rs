//! Block confirmation metadata for transactions and receipts.

use crate::TxLocation;
use alloy::primitives::{BlockNumber, B256};

/// Block confirmation metadata for a transaction or receipt.
///
/// Contains the block context in which a transaction was included,
/// enabling downstream consumers (e.g., RPC layers) to populate
/// block-related fields without additional lookups.
///
/// # Example
///
/// ```
/// # use alloy::primitives::B256;
/// # use signet_types::ConfirmationMeta;
/// let meta = ConfirmationMeta::new(42, B256::ZERO, 0);
/// assert_eq!(meta.block_number(), 42);
/// assert_eq!(meta.transaction_index(), 0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConfirmationMeta {
    /// The block number containing the transaction.
    block_number: BlockNumber,
    /// The hash of the block containing the transaction.
    block_hash: B256,
    /// The index of the transaction within the block.
    transaction_index: u64,
}

impl ConfirmationMeta {
    /// Create new confirmation metadata.
    pub const fn new(block_number: BlockNumber, block_hash: B256, transaction_index: u64) -> Self {
        Self { block_number, block_hash, transaction_index }
    }

    /// Returns the block number.
    pub const fn block_number(&self) -> BlockNumber {
        self.block_number
    }

    /// Returns the block hash.
    pub const fn block_hash(&self) -> B256 {
        self.block_hash
    }

    /// Returns the transaction index within the block.
    pub const fn transaction_index(&self) -> u64 {
        self.transaction_index
    }
}

impl From<(TxLocation, B256)> for ConfirmationMeta {
    fn from((loc, block_hash): (TxLocation, B256)) -> Self {
        Self::new(loc.block, block_hash, loc.index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confirmation_meta_new() {
        let meta = ConfirmationMeta::new(100, B256::ZERO, 5);
        assert_eq!(meta.block_number(), 100);
        assert_eq!(meta.block_hash(), B256::ZERO);
        assert_eq!(meta.transaction_index(), 5);
    }

    #[test]
    fn test_from_tx_location() {
        let loc = TxLocation { block: 42, index: 3 };
        let hash = B256::repeat_byte(0xAB);
        let meta = ConfirmationMeta::from((loc, hash));
        assert_eq!(meta.block_number(), 42);
        assert_eq!(meta.block_hash(), hash);
        assert_eq!(meta.transaction_index(), 3);
    }
}
