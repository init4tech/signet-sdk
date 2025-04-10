use crate::bindings::RollupOrders::{Output, Permit2Batch};
use serde::{Deserialize, Serialize};

/// An error that can occur when validating a signed order as a fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SignedOrderError {
    /// Mismatched permits and outputs.
    #[error("Permits and Outputs do not match.")]
    PermitMismatch(),
    /// The deadline for the order has passed.
    #[error("Deadline has passed: current time is: {current}, deadline was: {deadline}")]
    DeadlinePassed {
        /// The current timestamp.
        current: u64,
        /// The deadline for the [`Permit2Batch`].
        deadline: u64,
    },
}

/// A signed order.
/// TODO: Link to docs.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SignedOrder {
    /// The permit batch.
    #[serde(flatten)]
    pub permit: Permit2Batch,
    /// The desired outputs.
    pub outputs: Vec<Output>,
}

impl SignedOrder {
    /// Creates a new signed order.
    pub const fn new(permit: Permit2Batch, outputs: Vec<Output>) -> Self {
        Self { permit, outputs }
    }

    /// Check that this can be syntactically used as a fill.
    ///
    /// For it to be valid:
    /// - Deadline must be in the future.
    /// - The permits must exactly match the ordering, token, and amount of the outputs.
    pub fn validate_as_fill(&self, timestamp: u64) -> Result<(), SignedOrderError> {
        let deadline = self.permit.permit.deadline.saturating_to::<u64>();
        if timestamp > deadline {
            return Err(SignedOrderError::DeadlinePassed { current: timestamp, deadline });
        }

        // ensure Permits exactly match Outputs
        if self.outputs.len() != self.permit.permit.permitted.len() {
            return Err(SignedOrderError::PermitMismatch());
        }

        for (i, output) in self.outputs.iter().enumerate() {
            // check that the token is the same
            if output.token != self.permit.permit.permitted[i].token {
                return Err(SignedOrderError::PermitMismatch());
            }
            // check that the amount is exactly equal
            if output.amount != self.permit.permit.permitted[i].amount {
                return Err(SignedOrderError::PermitMismatch());
            }
        }

        Ok(())
    }
}
