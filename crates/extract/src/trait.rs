use alloy::{consensus::TxReceipt, primitives::Log};
use signet_types::primitives::{RecoveredBlock, TransactionSigned};

/// A trait for types from which data can be extracted. This currently exists
/// to provide a common interface for extracting data from host chain blocks
/// and receipts which may be in alloy or reth types.
pub trait Extractable: core::fmt::Debug + Sync {
    /// The block type that this extractor works with.
    type Block: alloy::consensus::BlockHeader + HasTxns + core::fmt::Debug + Sync;
    /// The receipt type that this extractor works with.
    type Receipt: TxReceipt<Log = Log> + core::fmt::Debug + Sync;

    /// An iterator over the blocks and their receipts.
    fn blocks_and_receipts(&self) -> impl Iterator<Item = (&Self::Block, &Vec<Self::Receipt>)>;
}

/// A trait for types that contain transactions. This currently exists to
/// provide a common interface for extracting data from host chain blocks and
/// receipts which may be in alloy or reth types.
pub trait HasTxns {
    /// Get the transactions in the block.
    fn transactions(&self) -> &[TransactionSigned];
}

impl HasTxns for signet_types::primitives::BlockBody {
    fn transactions(&self) -> &[TransactionSigned] {
        &self.transactions
    }
}

impl HasTxns for signet_types::primitives::SealedBlock {
    fn transactions(&self) -> &[TransactionSigned] {
        self.body.transactions.as_slice()
    }
}

impl HasTxns for RecoveredBlock {
    fn transactions(&self) -> &[TransactionSigned] {
        self.block.body.transactions.as_slice()
    }
}
