use crate::ExecutionOutcome;
use alloy::{consensus::Header, primitives::B256};
use signet_journal::{HostJournal, JournalMeta};
use signet_types::primitives::{RecoveredBlock, TransactionSigned};
use std::borrow::Cow;
use trevm::journal::BundleStateIndex;

/// Output of a block execution.
///
/// This is a convenience struct that combines the consensus block object with
/// the result of its execution.
#[derive(Debug, Default)]
pub struct BlockResult<T = TransactionSigned> {
    /// The host height.
    pub host_height: u64,

    /// A reth [`RecoveredBlock`], containing the sealed block and a vec of
    /// transaction senders.
    pub sealed_block: RecoveredBlock<T>,

    /// The reth [`ExecutionOutcome`] containing the net state changes and
    /// receipts.
    pub execution_outcome: ExecutionOutcome,
}

impl<T> BlockResult<T> {
    /// Create a new block result.
    pub const fn new(
        host_height: u64,
        sealed_block: RecoveredBlock<T>,
        execution_outcome: ExecutionOutcome,
    ) -> Self {
        Self { host_height, sealed_block, execution_outcome }
    }

    /// Get the rollup block header.
    pub const fn header(&self) -> &Header {
        self.sealed_block.block.header.inner()
    }

    /// Get the sealed block.
    pub const fn sealed_block(&self) -> &RecoveredBlock<T> {
        &self.sealed_block
    }

    /// Get the execution outcome.
    pub const fn execution_outcome(&self) -> &ExecutionOutcome {
        &self.execution_outcome
    }

    /// Calculate the [`BundleStateIndex`], making a sorted index of the
    /// contents of [`BundleState`] in the [`ExecutionOutcome`].
    ///
    /// [`BundleState`]: trevm::revm::database::BundleState
    pub fn index_bundle_state(&self) -> BundleStateIndex<'_> {
        BundleStateIndex::from(self.execution_outcome.bundle())
    }

    const fn journal_meta(&self, prev_journal_hash: B256) -> JournalMeta<'_> {
        JournalMeta::new(self.host_height, prev_journal_hash, Cow::Borrowed(self.header()))
    }

    /// Create a [`HostJournal`] by indexing the bundle state and block header.
    pub fn make_host_journal(&self, prev_journal_hash: B256) -> HostJournal<'_> {
        HostJournal::new(self.journal_meta(prev_journal_hash), self.index_bundle_state())
    }
}
