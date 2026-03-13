use alloy::consensus::{BlockHeader, TxReceipt};
use alloy::primitives::Log;
use signet_types::primitives::TransactionSigned;

/// A block with its associated receipts.
#[derive(Debug)]
pub struct BlockAndReceipts<'a, B, R> {
    /// The block.
    pub block: &'a B,
    /// The receipts for this block's transactions.
    pub receipts: &'a [R],
}

/// A trait for types from which data can be extracted. This currently exists
/// to provide a common interface for extracting data from host chain blocks
/// and receipts which may be in alloy or reth types.
pub trait Extractable: core::fmt::Debug + Sync {
    /// The block type that this extractor works with.
    type Block: alloy::consensus::BlockHeader + HasTxns + core::fmt::Debug + Sync;
    /// The receipt type that this extractor works with.
    type Receipt: TxReceipt<Log = Log> + core::fmt::Debug + Sync;

    /// An iterator over the blocks and their receipts.
    fn blocks_and_receipts(
        &self,
    ) -> impl Iterator<Item = BlockAndReceipts<'_, Self::Block, Self::Receipt>>;

    /// Block number of the first block in the segment, or `None` if empty.
    fn first_number(&self) -> Option<u64> {
        self.blocks_and_receipts().next().map(|bar| bar.block.number())
    }

    /// Block number of the tip (last block) in the segment, or `None` if
    /// empty.
    fn tip_number(&self) -> Option<u64> {
        self.blocks_and_receipts().last().map(|bar| bar.block.number())
    }

    /// Number of blocks in the segment.
    fn len(&self) -> usize {
        self.blocks_and_receipts().count()
    }

    /// Whether the segment is empty.
    fn is_empty(&self) -> bool {
        self.blocks_and_receipts().next().is_none()
    }
}

/// A trait for types that contain transactions. This currently exists to
/// provide a common interface for extracting data from host chain blocks and
/// receipts which may be in alloy or reth types.
pub trait HasTxns {
    /// Get the transactions in the block.
    fn transactions(&self) -> impl ExactSizeIterator<Item = &TransactionSigned>;
}

impl<T: AsRef<TransactionSigned>> HasTxns for signet_types::primitives::SealedBlock<T> {
    fn transactions(&self) -> impl ExactSizeIterator<Item = &TransactionSigned> {
        self.transactions.iter().map(AsRef::as_ref)
    }
}
