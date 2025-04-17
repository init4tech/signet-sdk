use super::AggregateOrders;
use crate::bindings::RollupOrders::{Order, Output, Permit2Batch};
use alloy::primitives::Address;
use alloy::signers::Signer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

    /// Check that this can be syntactically used to initiate an order.
    ///
    /// For it to be valid:
    /// - Deadline must be in the future.
    pub fn validate(&self, timestamp: u64) -> Result<(), SignedPermitError> {
        let deadline = self.permit.permit.deadline.saturating_to::<u64>();
        if timestamp > deadline {
            return Err(SignedPermitError::DeadlinePassed { current: timestamp, deadline });
        }

        Ok(())
    }
}

/// A signed fill for one or more Orders on a given destination chain.
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

/// An error that can occur when signing an Order or a Fill.
#[derive(Debug, thiserror::Error)]
pub enum SignerError {
    /// Missing chain config.
    #[error("Target chain id and Order contract address are missing. Populate them by calling with_chain before attempting to sign.")]
    MissingChainConfig,
    /// Missing chain config for a specific chain.
    #[error("Target chain id and Order contract address are missing for chain id {0}. Populate them by calling with_chain before attempting to sign.")]
    MissingChainConfigFor(u64),
    /// Error signing the order hash.
    #[error("Signer error: {0}")]
    Signer(#[from] alloy::signers::Error),
}

/// An unsigned order. Used to turn an Order into a SignedOrder.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct UnsignedOrder<'a> {
    order: std::borrow::Cow<'a, Order>,
    nonce: Option<u64>,
    rollup_chain_id: Option<u64>,
    rollup_order_address: Option<Address>,
}

impl<'a> From<&'a Order> for UnsignedOrder<'a> {
    fn from(order: &'a Order) -> Self {
        UnsignedOrder::new(order)
    }
}

impl<'a> UnsignedOrder<'a> {
    /// Get a new UnsignedOrder from an Order.
    pub fn new(order: &'a Order) -> Self {
        Self { order: order.into(), nonce: None, rollup_chain_id: None, rollup_order_address: None }
    }

    /// Add a Permit2 nonce to the UnsignedOrder.
    pub fn with_nonce(self, nonce: u64) -> Self {
        Self { nonce: Some(nonce), ..self }
    }

    /// Add the chain id  and Order contract address to the UnsignedOrder.
    pub fn with_chain(self, chain_id: u64, order_contract_address: Address) -> Self {
        Self {
            rollup_chain_id: Some(chain_id),
            rollup_order_address: Some(order_contract_address),
            ..self
        }
    }

    /// Sign the UnsignedOrder, generating a SignedOrder.
    pub async fn sign<S: Signer>(&self, signer: &S) -> Result<SignedOrder, SignerError> {
        // if nonce is None, populate it with the current time
        let nonce = self.nonce.unwrap_or(chrono::Utc::now().timestamp_millis() as u64);

        // construct the Permit2 signing hash & sign it
        let signing_hash = self.order.initiate_signing_hash(
            nonce,
            self.rollup_chain_id.ok_or(SignerError::MissingChainConfig)?,
            self.rollup_order_address.ok_or(SignerError::MissingChainConfig)?,
        );
        let signature = signer.sign_hash(&signing_hash).await?;

        Ok(SignedOrder {
            permit: Permit2Batch {
                permit: self.order.initiate_permit(nonce),
                owner: signer.address(),
                signature: signature.as_bytes().into(),
            },
            outputs: self.order.outputs().to_vec(),
        })
    }
}

/// An unsigned fill. Used to turn AggregateOrders into a SignedFill.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct UnsignedFill<'a> {
    orders: std::borrow::Cow<'a, AggregateOrders>,
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
    /// Utilizes the same signing key for every chain.
    pub async fn sign<S: Signer>(
        &self,
        signer: &S,
    ) -> Result<HashMap<u64, SignedFill>, SignerError> {
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
    /// NOTE that *all* Outputs MUST be filled on all chains, else Inputs will not be transferred.
    pub async fn sign_for<S: Signer>(
        &self,
        chain_id: u64,
        signer: &S,
    ) -> Result<SignedFill, SignerError> {
        let now = chrono::Utc::now();
        // if nonce is are None, populate it as the current timestamp in milliseconds
        let nonce = self.nonce.unwrap_or(now.timestamp_millis() as u64);
        // if deadline is None, populate it as now + 12 seconds (can only mine within the current block)
        let deadline = self.deadline.unwrap_or(now.timestamp() as u64 + 12);

        let destination_order_address = self
            .destination_chains
            .get(&chain_id)
            .ok_or(SignerError::MissingChainConfigFor(chain_id))?;

        let signing_hash =
            self.orders.fill_signing_hash(deadline, nonce, chain_id, *destination_order_address);
        let signature = signer.sign_hash(&signing_hash).await?;

        Ok(SignedFill {
            permit: Permit2Batch {
                permit: self.orders.fill_permit(deadline, nonce, chain_id),
                owner: signer.address(),
                signature: signature.as_bytes().into(),
            },
            outputs: self.orders.outputs_for(chain_id).to_vec(),
        })
    }
}
