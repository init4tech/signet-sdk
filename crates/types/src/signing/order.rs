use crate::signing::{permit_signing_info, SignedPermitError, SigningError};
use alloy::{
    network::TransactionBuilder,
    primitives::{Address, B256},
    rpc::types::TransactionRequest,
    signers::Signer,
    sol_types::{SolCall, SolValue},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use signet_zenith::RollupOrders::{
    initiatePermit2Call, Order, Output, Permit2Batch, TokenPermissions,
};
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

    /// Generate a TransactionRequest to `initiate` the SignedOrder.
    pub fn to_initiate_tx(
        &self,
        filler_token_recipient: Address,
        order_contract: Address,
    ) -> TransactionRequest {
        // encode initiate data
        let initiate_data = initiatePermit2Call {
            tokenRecipient: filler_token_recipient,
            outputs: self.outputs.clone(),
            permit2: self.permit.clone(),
        }
        .abi_encode();

        // construct an initiate tx request
        TransactionRequest::default().with_input(initiate_data).with_to(order_contract)
    }

    /// Get the hash of the order.
    ///
    /// # Composition
    ///
    /// The order hash is composed of the following:
    /// - The permit2 batch permit inputs, ABI encoded.
    /// - The permit2 batch owner, ABI encoded.
    /// - The order outputs, ABI encoded.
    /// - The permit2 batch signature, normalized.
    ///
    /// The components are then hashed together.
    pub fn order_hash(&self) -> B256 {
        let mut buf = vec![];

        buf.extend_from_slice(self.permit.permit.abi_encode().as_slice());
        buf.extend_from_slice(self.permit.owner.abi_encode().as_slice());
        buf.extend_from_slice(self.outputs.abi_encode().as_slice());

        // Normalize the signature.
        let signature =
            alloy::primitives::Signature::from_raw(&self.permit.signature).unwrap().normalized_s();
        buf.extend_from_slice(&signature.as_bytes());

        alloy::primitives::keccak256(buf)
    }
}

/// An UnsignedOrder is a helper type used to easily transform an Order into a SignedOrder with correct permit2 semantics.
/// Users can do:
/// let signed_order = UnsignedOrder::from(order).with_chain(rollup_chain_id, rollup_order_address).sign(signer)?;
/// TxCache::new(tx_cache_endpoint).forward_order(signed_order);
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

        // get chain id and order contract address
        let rollup_chain_id = self.rollup_chain_id.ok_or(SigningError::MissingChainId)?;
        let rollup_order_contract =
            self.rollup_order_address.ok_or(SigningError::MissingOrderContract(rollup_chain_id))?;

        // get the outputs for the Order
        let outputs = self.order.outputs().to_vec();
        // generate the permitted tokens from the Inputs on the Order
        let permitted: Vec<TokenPermissions> = self.order.inputs().iter().map(Into::into).collect();

        // generate the permit2 signing info
        let permit = permit_signing_info(
            outputs,
            permitted,
            self.order.deadline(),
            nonce,
            rollup_chain_id,
            rollup_order_contract,
        );

        // sign it
        let signature = signer.sign_hash(&permit.signing_hash).await?;

        // return as a SignedOrder
        Ok(SignedOrder {
            permit: Permit2Batch {
                permit: permit.permit,
                owner: signer.address(),
                signature: signature.as_bytes().into(),
            },
            outputs: permit.outputs,
        })
    }
}
