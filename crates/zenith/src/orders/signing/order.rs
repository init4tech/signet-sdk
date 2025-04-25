use crate::{
    bindings::RollupOrders::{Order, Output, Permit2Batch},
    orders::signing::{SignedPermitError, SigningError},
};
use alloy::{primitives::Address, signers::Signer};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// A SignedOrder represents a single Order after it has been permit2-encoded and signed.
/// It is the final format signed by Users and shared with Fillers to request that an Order be filled.
///
/// It corresponds to the parameters for `initiatePermit2` on the OrderOrigin contract,
/// and thus contains all necessary information to initiate the Order.
///
/// It can be shared with all Fillers via the Signet Node `signet_sendOrder` RPC call,
/// or shared directly with specific Filler(s) via private channels.
/// The type can be signed and published safely, because although the Permit2Batch allows
/// the Order Inputs to be transferred from the user, the Signet Node ensures that
/// Inputs cannot be transferred until the Order Outputs have already been filled.
///
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
/// Users can do: 
/// let signed_order = UnsignedOrder::from(order).with_chain(rollup_chain_id, rollup_order_address).sign(signer)?;
/// TxCacheForwarder::new(tx_cache_endpoint).send_order(signed_order);
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
