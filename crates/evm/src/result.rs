use reth::{
    primitives::{Block, RecoveredBlock},
    providers::ExecutionOutcome,
};

/// Output of a block execution.
///
/// This is a convenience struct that combines the consensus block object with
/// the result of its execution.
#[derive(Debug, Default)]
pub struct BlockResult {
    /// A reth [`RecoveredBlock`], containing the sealed block and a vec of
    /// transaction sender.
    pub sealed_block: RecoveredBlock<Block>,
    /// The reth [`ExecutionOutcome`] containing the net state changes and
    /// receipts.
    pub execution_outcome: ExecutionOutcome,
}
