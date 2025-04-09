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

        // Check that
        let mut map = HashMap::with_capacity(self.outputs.len());

        for output in self.outputs.iter() {
            map.insert(output.token, output.amount);
        }

        // We now remove the amounts of the permits from the map
        // if the resulting amount is zero, we remove the entry. This
        // means that the output is satisfied by the permit.
        for permit in self.permit.permit.permitted.iter() {
            match map.entry(permit.token) {
                Entry::Occupied(mut occupied_entry) => {
                    let val = occupied_entry.get();
                    let update = val.saturating_sub(permit.amount);
                    if update.is_zero() {
                        occupied_entry.remove();
                    } else {
                        occupied_entry.insert(update);
                    }
                }
                _ => {}
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
