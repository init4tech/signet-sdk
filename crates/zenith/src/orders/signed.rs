use crate::bindings::RollupOrders::{Order, Output, Permit2Batch};
use alloy::primitives::{Address, U256};
use alloy::signers::Signer;
use serde::{Deserialize, Serialize};

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
    /// Missing chain id.
    #[error("Chain id must be populated using with_chain_id.")]
    MissingChainId,
    /// Missing Order contract address.
    #[error("Order contract address must be populated using with_order_contract.")]
    MissingOrderContract,
    /// Error signing the order hash.
    #[error("Signer error: {0}")]
    Signer(#[from] alloy::signers::Error),
}

/// An unsigned order. Used to turn an Order into a SignedOrder.
/// E.g. let SignedOrder = UnsignedOrder::from(order).with_chain_id(chain_id).with_order_contract(address).sign(&signer).await?;
pub struct UnsignedOrder<'a> {
    order: std::borrow::Cow<'a, Order>,
    nonce: Option<u64>,
    rollup_chain_id: Option<U256>,
    rollup_order_address: Option<Address>,
}

impl<'a> From<&'a Order> for UnsignedOrder<'a> {
    fn from(order: &'a Order) -> Self {
        UnsignedOrder::new(order)
    }
}

impl<'a> UnsignedOrder<'a> {
    pub fn new(order: &'a Order) -> Self {
        Self { order: order.into(), nonce: None, rollup_chain_id: None, rollup_order_address: None }
    }

    /// Add a nonce to the UnsignedOrder.
    pub fn with_nonce(self, nonce: u64) -> Self {
        Self { nonce: Some(nonce), ..self }
    }

    /// Add a chain id to the UnsignedOrder.
    pub fn with_chain_id(self, rollup_chain_id: U256) -> Self {
        Self { rollup_chain_id: Some(rollup_chain_id), ..self }
    }

    /// Add the rollup order contract address to the UnsignedOrder.
    pub fn with_order_contract(self, address: Address) -> Self {
        Self { rollup_order_address: Some(address), ..self }
    }

    /// Sign the UnsignedOrder, generating a SignedOrder.
    pub async fn sign<S: Signer>(&self, signer: &S) -> Result<SignedOrder, SignerError> {
        // if nonce is None, populate it with the current time
        let nonce = U256::from(self.nonce.unwrap_or(chrono::Utc::now().timestamp_millis() as u64));

        // construct the Permit2 signing hash & sign it
        let signing_hash = self.order.initiate_signing_hash(
            nonce,
            self.rollup_chain_id.ok_or(SignerError::MissingChainId)?,
            self.rollup_order_address.ok_or(SignerError::MissingOrderContract)?,
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
