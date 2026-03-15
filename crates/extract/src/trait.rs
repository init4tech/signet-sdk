use alloy::{
    consensus::{BlockHeader, TxReceipt},
    primitives::Log,
};
use signet_types::primitives::TransactionSigned;

/// A block with its associated receipts, yielded by
/// [`Extractable::blocks_and_receipts`].
///
/// ```
/// # use alloy::consensus::BlockHeader;
/// # use signet_extract::BlockAndReceipts;
/// # fn example(bar: BlockAndReceipts<'_, alloy::consensus::Header, alloy::consensus::ReceiptEnvelope>) {
/// let _number = bar.block.number();
/// let _count = bar.receipts.len();
/// # }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct BlockAndReceipts<'a, B, R> {
    /// The block.
    pub block: &'a B,
    /// The receipts for this block's transactions.
    pub receipts: &'a [R],
}

/// A trait for types from which data can be extracted. This currently exists
/// to provide a common interface for extracting data from host chain blocks
/// and receipts which may be in alloy or reth types.
///
/// Implementors must guarantee that the segment is non-empty — i.e.,
/// [`blocks_and_receipts`] always yields at least one item. An empty
/// extractable segment is not meaningful.
///
/// [`blocks_and_receipts`]: Extractable::blocks_and_receipts
#[allow(clippy::len_without_is_empty)]
pub trait Extractable: core::fmt::Debug + Sync {
    /// The block type that this extractor works with.
    type Block: alloy::consensus::BlockHeader + HasTxns + core::fmt::Debug + Sync;
    /// The receipt type that this extractor works with.
    type Receipt: TxReceipt<Log = Log> + core::fmt::Debug + Sync;

    /// An iterator over the blocks and their receipts.
    ///
    /// Blocks must be yielded in ascending order by block number. The
    /// iterator must yield at least one item.
    fn blocks_and_receipts(
        &self,
    ) -> impl Iterator<Item = BlockAndReceipts<'_, Self::Block, Self::Receipt>>;

    /// Block number of the first block in the segment.
    fn first_number(&self) -> u64 {
        self.blocks_and_receipts().next().expect("Extractable must be non-empty").block.number()
    }

    /// Block number of the tip (last block) in the segment.
    ///
    /// The default implementation consumes the entire iterator. Implementors
    /// with indexed access should override this.
    fn tip_number(&self) -> u64 {
        self.blocks_and_receipts().last().expect("Extractable must be non-empty").block.number()
    }

    /// Number of blocks in the segment. Always `>= 1`.
    ///
    /// The default implementation consumes the entire iterator. Implementors
    /// that know their length should override this.
    fn len(&self) -> usize {
        self.blocks_and_receipts().count()
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
