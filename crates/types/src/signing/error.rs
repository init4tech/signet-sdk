/// An error that can occur when validating a signed order or fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SignedPermitError {
    /// Mismatched permits and outputs.
    #[error("Permits and Outputs do not match.")]
    PermitMismatch,
    /// The deadline for the order has passed.
    #[error("Deadline has passed: current time is: {current}, deadline was: {deadline}")]
    DeadlinePassed {
        /// The current timestamp.
        current: u64,
        /// The deadline for the [`Permit2Batch`].
        deadline: u64,
    },
}

/// An error that can occur when signing an Order or a Fill.
#[derive(Debug, thiserror::Error)]
pub enum SigningError {
    /// Missing chain config.
    #[error(
        "Target chain id is missing. Populate it by calling with_chain before attempting to sign"
    )]
    MissingChainId,
    /// Missing rollup chain id for a Fill.
    #[error(
        "Rollup chain id is missing. Populate it by calling with_chain before attempting to sign"
    )]
    #[deprecated(since = "0.14.1", note = "Use MissingChainId instead.")]
    MissingRollupChainId,
    /// Missing chain config for a specific chain.
    #[error("Target Order contract address is missing for chain id {0}. Populate it by calling with_chain before attempting to sign")]
    MissingOrderContract(u64),
    /// Error signing the order hash.
    #[error(transparent)]
    Signer(#[from] alloy::signers::Error),
}
