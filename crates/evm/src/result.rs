use reth::{
    primitives::{Block, RecoveredBlock},
    providers::ExecutionOutcome,
};

/// Output of a block execution
#[derive(Debug, Default)]
pub struct BlockResult {
    /// Sealed block with senders
    pub sealed_block: RecoveredBlock<Block>,
    /// Bundle state with receipts
    pub execution_outcome: ExecutionOutcome,
}
