use crate::bindings::RollupOrders::{Output, Permit2Batch};
use alloy::primitives::Address;
use serde::{Deserialize, Serialize};
use std::collections::{hash_map::Entry, HashMap};

/// An error that can occur when validating a signed order as a fill.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SignedOrderError {
    /// Some assets had insufficient permits.
    #[error("Insufficient permits for some assets")]
    Insufficient(Vec<Address>),
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
    /// - The value of the permits must be greater than or equal the values of
    ///   the outputs.
    /// - Deadline must be in the future.
    pub fn validate_as_fill(&self, timestamp: u64) -> Result<(), SignedOrderError> {
        let deadline = self.permit.permit.deadline.saturating_to::<u64>();
        if timestamp > deadline {
            return Err(SignedOrderError::DeadlinePassed { current: timestamp, deadline });
        }

        // Check that the permits satisfy the outputs.
        // We create a map of the outputs, where the key is the token and the
        // value is the amount. We then iterate over the permits and remove
        // the amount from the map. If the amount is zero, we remove the entry
        // from the map. If the map is not empty, then we have some tokens that
        // are not satisfied by the permits.
        let mut map = HashMap::with_capacity(self.outputs.len());

        for output in self.outputs.iter() {
            map.insert(output.token, output.amount);
        }

        for permit in self.permit.permit.permitted.iter() {
            if let Entry::Occupied(mut occupied_entry) = map.entry(permit.token) {
                let val = occupied_entry.get();
                let update = val.saturating_sub(permit.amount);
                if update.is_zero() {
                    occupied_entry.remove();
                } else {
                    occupied_entry.insert(update);
                }
            }
        }

        // If the map is not empty, then we have some tokens that are not
        // satisfied by the permits.
        if !map.is_empty() {
            return Err(SignedOrderError::Insufficient(map.keys().copied().collect()));
        }

        Ok(())
    }
}
