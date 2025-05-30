use alloy::consensus::ReceiptEnvelope;
use trevm::revm::database::BundleState;

/// The outcome of a block execution, containing the bundle state,
/// receipts, and the first block number in the execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionOutcome<T = ReceiptEnvelope> {
    /// The bundle state after execution.
    pub bundle: BundleState,
    /// The receipts generated during execution, grouped by block.
    pub receipts: Vec<Vec<T>>,
    /// The first block number in the execution.
    pub first_block: u64,
}

impl<T> Default for ExecutionOutcome<T> {
    fn default() -> Self {
        Self { bundle: BundleState::default(), receipts: vec![], first_block: 0 }
    }
}

impl<T> ExecutionOutcome<T> {
    /// Create a new execution outcome.
    pub const fn new(bundle: BundleState, receipts: Vec<Vec<T>>, first_block: u64) -> Self {
        Self { bundle, receipts, first_block }
    }

    /// Append another execution outcome to this one.
    pub fn append(&mut self, other: Self) {
        self.bundle.extend(other.bundle);
        self.receipts.extend(other.receipts);
    }
}
