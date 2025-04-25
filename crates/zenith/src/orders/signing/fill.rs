use crate::{
    bindings::RollupOrders::{Output, Permit2Batch},
    orders::{
        signing::{SignedPermitError, SigningError},
        AggregateOrders,
    },
};
use alloy::{primitives::Address, signers::Signer};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, collections::HashMap};

/// A single SignedFill contains the aggregated Outputs to fill any number of Orders on a single destination chain.
/// The type corresponds to the parameters for `fillPermit2` on the OrderDestination contract on a given chain.
/// The Permit2Batch is signed by the Filler, allowing the Order Outputs to be transferred from the Filler to their recipients.
/// # Warning ⚠️
/// A SignedFill *must* remain private until it is mined, as there is no guarantee in the OrderDestination contract that desired Order Inputs will be received in return for the Fill.
/// It is important to use private transaction relays to send the SignedFill to Builders, both on the rollup and host chains.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SignedFill {
    /// The permit batch.
    #[serde(flatten)]
    pub permit: Permit2Batch,
    /// The desired outputs.
    pub outputs: Vec<Output>,
}

impl SignedFill {
    /// Creates a new signed fill.
    pub const fn new(permit: Permit2Batch, outputs: Vec<Output>) -> Self {
        Self { permit, outputs }
    }

    /// Check that this can be syntactically used as a fill.
    ///
    /// For it to be valid:
    /// - Deadline must be in the future.
    /// - The permits must exactly match the ordering, token, and amount of the outputs.
    pub fn validate(&self, timestamp: u64) -> Result<(), SignedPermitError> {
        let deadline = self.permit.permit.deadline.saturating_to::<u64>();
        if timestamp > deadline {
            return Err(SignedPermitError::DeadlinePassed { current: timestamp, deadline });
        }

        // ensure Permits exactly match Outputs
        if self.outputs.len() != self.permit.permit.permitted.len() {
            return Err(SignedPermitError::PermitMismatch);
        }

        for (output, permit) in self.outputs.iter().zip(self.permit.permit.permitted.iter()) {
            // check that the token is the same
            if output.token != permit.token {
                return Err(SignedPermitError::PermitMismatch);
            }
            // check that the amount is exactly equal
            if output.amount != permit.amount {
                return Err(SignedPermitError::PermitMismatch);
            }
        }

        Ok(())
    }
}

/// An UnsignedFill is a helper type used to easily transform an AggregateOrder into a single SignedFill per target chain with correct permit2 semantics.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct UnsignedFill<'a> {
    orders: Cow<'a, AggregateOrders>,
    deadline: Option<u64>,
    nonce: Option<u64>,
    destination_chains: HashMap<u64, Address>,
}

impl<'a> From<&'a AggregateOrders> for UnsignedFill<'a> {
    fn from(orders: &'a AggregateOrders) -> Self {
        UnsignedFill::new(orders)
    }
}

impl<'a> UnsignedFill<'a> {
    /// Get a new UnsignedFill from a set of AggregateOrders.
    pub fn new(orders: &'a AggregateOrders) -> Self {
        Self {
            orders: orders.into(),
            deadline: None,
            nonce: None,
            destination_chains: HashMap::new(),
        }
    }

    /// Add a Permit2 nonce to the UnsignedFill.
    pub fn with_nonce(self, nonce: u64) -> Self {
        Self { nonce: Some(nonce), ..self }
    }

    /// Add a deadline to the UnsignedFill, after which it cannot be mined.
    pub fn with_deadline(self, deadline: u64) -> Self {
        Self { deadline: Some(deadline), ..self }
    }

    /// Add the chain id  and Order contract address to the UnsignedOrder.
    pub fn with_chain(mut self, chain_id: u64, order_contract_address: Address) -> Self {
        self.destination_chains.insert(chain_id, order_contract_address);
        self
    }

    /// Sign the UnsignedFill, generating a SignedFill for each destination chain.
    /// Use if Filling Orders with the same signing key on every chain.
    pub async fn sign<S: Signer>(
        &self,
        signer: &S,
    ) -> Result<HashMap<u64, SignedFill>, SigningError> {
        let mut fills = HashMap::new();

        // loop through each destination chain and sign the fills
        for destination_chain_id in self.orders.output_chain_ids() {
            let signed_fill = self.sign_for(destination_chain_id, signer).await?;
            fills.insert(destination_chain_id, signed_fill);
        }

        // return the fills
        Ok(fills)
    }

    /// Sign the UnsignedFill for a specific destination chain.
    /// Use if Filling Orders with different signing keys on respective destination chains.
    /// # Warning ⚠️
    /// *All* Outputs MUST be filled on all destination chains, else the Order Inputs will not be transferred.
    /// Take care when using this function to produce SignedFills for every destination chain.
    pub async fn sign_for<S: Signer>(
        &self,
        chain_id: u64,
        signer: &S,
    ) -> Result<SignedFill, SigningError> {
        let now = Utc::now();
        // if nonce is are None, populate it as the current timestamp in milliseconds
        let nonce = self.nonce.unwrap_or(now.timestamp_micros() as u64);
        // if deadline is None, populate it as now + 12 seconds (can only mine within the current block)
        let deadline = self.deadline.unwrap_or(now.timestamp() as u64 + 12);

        let destination_order_address = self
            .destination_chains
            .get(&chain_id)
            .ok_or(SigningError::MissingOrderContract(chain_id))?;

        let signing_hash =
            self.orders.fill_signing_hash(deadline, nonce, chain_id, *destination_order_address);
        let signature = signer.sign_hash(&signing_hash).await?;

        Ok(SignedFill {
            permit: Permit2Batch {
                permit: self.orders.to_fill_permit(deadline, nonce, chain_id),
                owner: signer.address(),
                signature: signature.as_bytes().into(),
            },
            outputs: self.orders.outputs_for(chain_id).to_vec(),
        })
    }
}
