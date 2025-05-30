use crate::{journal::HostJournal, ExecutionOutcome};
use alloy::{consensus::Header, primitives::B256};
use signet_types::primitives::{RecoveredBlock, TransactionSigned};
use trevm::journal::BundleStateIndex;

/// Output of a block execution.
///
/// This is a convenience struct that combines the consensus block object with
/// the result of its execution.
#[derive(Debug, Default)]
pub struct BlockResult<T = TransactionSigned, H = Header> {
    /// A reth [`RecoveredBlock`], containing the sealed block and a vec of
    /// transaction sender.
    pub sealed_block: RecoveredBlock<T, H>,
    /// The reth [`ExecutionOutcome`] containing the net state changes and
    /// receipts.
    pub execution_outcome: ExecutionOutcome,
}

impl<T, H> BlockResult<T, H> {
    /// Create a new block result.
    pub const fn new(
        sealed_block: RecoveredBlock<T, H>,
        execution_outcome: ExecutionOutcome,
    ) -> Self {
        Self { sealed_block, execution_outcome }
    }

    /// Get the sealed block.
    pub const fn sealed_block(&self) -> &RecoveredBlock<T, H> {
        &self.sealed_block
    }

    /// Get the execution outcome.
    pub const fn execution_outcome(&self) -> &ExecutionOutcome {
        &self.execution_outcome
    }

    /// Make a journal of the block result. This indexes the bundle state.
    pub fn make_journal(&self, host_block: u64, prev_journal_hash: B256) -> HostJournal<'_> {
        HostJournal::new(
            host_block,
            prev_journal_hash,
            BundleStateIndex::from(&self.execution_outcome.bundle),
        )
    }
}
