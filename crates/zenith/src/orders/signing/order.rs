use crate::{
    bindings::RollupOrders::{Order, Output, Permit2Batch},
    orders::signing::{SignedPermitError, SigningError},
};
use alloy::{primitives::Address, signers::Signer};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// A SignedOrder contains the information for a single Order, after it has been correctly permit2-encoded and signed by a User.
/// The type corresponds to the parameters for `initiatePermit2` on the OrderOrigin contract on the rollup.
/// The Permit2Batch is signed by the User, allowing the Order Inputs to be transferred from the user to the Filler.
/// The Outputs the user expects to receive in return are listed explicitly, as well as committed to in the Permit2Batch signature.
/// Users can sign an Order for any swap they are willing to make safely,
/// as the Inputs cannot be transferred until the Outputs have already been delivered to the specified recipients.
/// A SignedOrder can be shared directly with Fillers, or forwarded to a Signet Node to become publicly available to be filled.
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

/// An UnsignedOrder is a helper type used to easily transform an Order into a SignedOrder with correct permit2 semantics.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct UnsignedOrder<'a> {
    order: Cow<'a, Order>,
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
    pub async fn sign<S: Signer>(&self, signer: &S) -> Result<SignedOrder, SigningError> {
        // if nonce is None, populate it with the current time
        let nonce = self.nonce.unwrap_or(Utc::now().timestamp_micros() as u64);

        let rollup_chain_id = self.rollup_chain_id.ok_or(SigningError::MissingChainId)?;
        let rollup_order_contract =
            self.rollup_order_address.ok_or(SigningError::MissingOrderContract(rollup_chain_id))?;

        // construct the Permit2 signing hash & sign it
        let signing_hash =
            self.order.initiate_signing_hash(nonce, rollup_chain_id, rollup_order_contract);
        let signature = signer.sign_hash(&signing_hash).await?;

        Ok(SignedOrder {
            permit: Permit2Batch {
                permit: self.order.to_initiate_permit(nonce),
                owner: signer.address(),
                signature: signature.as_bytes().into(),
            },
            outputs: self.order.outputs().to_vec(),
        })
    }
}
