use crate::signing::{permit_signing_info, SignedPermitError, SigningError};
use alloy::{
    network::TransactionBuilder,
    primitives::{keccak256, Address, B256, U256},
    rpc::types::TransactionRequest,
    signers::Signer,
    sol_types::{SolCall, SolValue},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use signet_constants::SignetSystemConstants;
use signet_zenith::RollupOrders::{
    initiatePermit2Call, Input, Order, Output, Permit2Batch, TokenPermissions,
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
    /// - The permit2 batch permit inputs, ABI encoded and hashed.
    /// - The permit2 batch owner, ABI encoded and hashed.
    /// - The order outputs, ABI encoded and hashed.
    /// - The permit2 batch signature, normalized and hashed.
    ///
    /// The components are then hashed together.
    pub fn order_hash(&self) -> B256 {
        keccak256(self.order_hash_pre_image())
    }

    /// Get the pre-image for the order hash.
    ///
    /// This is the raw bytes that are hashed to produce the order hash.
    #[doc(hidden)]
    pub fn order_hash_pre_image(&self) -> Vec<u8> {
        // 4 * 32 bytes = 128 bytes
        let mut buf = Vec::with_capacity(128);

        buf.extend_from_slice(keccak256(self.permit.permit.abi_encode()).as_slice());
        buf.extend_from_slice(keccak256(self.permit.owner.abi_encode()).as_slice());
        buf.extend_from_slice(keccak256(self.outputs.abi_encode()).as_slice());

        // Normalize the signature.
        let signature =
            alloy::primitives::Signature::from_raw(&self.permit.signature).unwrap().normalized_s();
        buf.extend_from_slice(keccak256(signature.as_bytes()).as_slice());

        buf
    }
}

/// An UnsignedOrder is a helper type used to easily transform an Order into a
/// SignedOrder with correct permit2 semantics.
/// Users can do:
/// let signed_order = UnsignedOrder::from(order).with_chain(rollup_chain_id, rollup_order_address).sign(signer)?;
/// TxCache::new(tx_cache_endpoint).forward_order(signed_order);
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct UnsignedOrder<'a> {
    order: Cow<'a, Order>,
    nonce: Option<u64>,
    rollup_chain_id: Option<u64>,
    rollup_order_address: Option<Address>,
}

impl<'a> From<&'a Order> for UnsignedOrder<'a> {
    fn from(order: &'a Order) -> Self {
        Self { order: Cow::Borrowed(order), ..Default::default() }
    }
}

impl<'a> UnsignedOrder<'a> {
    /// Get a new UnsignedOrder from an Order.
    pub fn new() -> Self {
        Self {
            order: Cow::Owned(Order::default()),
            nonce: None,
            rollup_chain_id: None,
            rollup_order_address: None,
        }
    }

    /// Get the inputs of the UnsignedOrder.
    pub fn inputs(&self) -> &[Input] {
        self.order.inputs()
    }

    /// Add an input to the UnsignedOrder.
    pub fn with_raw_input(self, input: Input) -> UnsignedOrder<'static> {
        let order = self.order.into_owned().with_input(input);

        UnsignedOrder { order: Cow::Owned(order), ..self }
    }

    /// Add an input to the UnsignedOrder.
    pub fn with_input(self, token: Address, amount: U256) -> UnsignedOrder<'static> {
        self.with_raw_input(Input { token, amount })
    }

    /// Get the outputs of the UnsignedOrder.
    pub fn outputs(&self) -> &[Output] {
        self.order.outputs()
    }

    /// Add an output to the UnsignedOrder.
    pub fn with_raw_output(self, output: Output) -> UnsignedOrder<'static> {
        let order = self.order.into_owned().with_output(output);

        UnsignedOrder { order: Cow::Owned(order), ..self }
    }

    /// Add an output to the UnsignedOrder.
    pub fn with_output(
        self,
        token: Address,
        amount: U256,
        recipient: Address,
        chain_id: u32,
    ) -> UnsignedOrder<'static> {
        self.with_raw_output(Output { token, amount, recipient, chainId: chain_id })
    }

    /// Set the deadline on the UnsignedOrder.
    pub fn with_deadline(self, deadline: u64) -> UnsignedOrder<'static> {
        let order = self.order.into_owned().with_deadline(deadline);

        UnsignedOrder { order: Cow::Owned(order), ..self }
    }

    /// Add a Permit2 nonce to the UnsignedOrder.
    pub fn with_nonce(self, nonce: u64) -> Self {
        Self { nonce: Some(nonce), ..self }
    }

    /// Add the chain id  and Order contract address to the UnsignedOrder.
    /// MUST call before `sign`.
    pub fn with_chain(self, constants: &SignetSystemConstants) -> Self {
        Self {
            rollup_chain_id: Some(constants.ru_chain_id()),
            rollup_order_address: Some(constants.ru_orders()),
            ..self
        }
    }

    /// Convert the UnsignedOrder into an Order, cloning the inner data if
    /// necessary.
    pub fn to_order(&self) -> Order {
        self.order.clone().into_owned()
    }

    /// Convert the UnsignedOrder into an Order
    pub fn into_order(self) -> Cow<'a, Order> {
        self.order
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

#[cfg(test)]
mod tests {
    use alloy::primitives::{b256, Signature, U256};
    use signet_zenith::HostOrders::{PermitBatchTransferFrom, TokenPermissions};

    use super::*;

    fn basic_order() -> SignedOrder {
        SignedOrder::new(
            Permit2Batch {
                permit: PermitBatchTransferFrom {
                    permitted: vec![TokenPermissions { token: Address::ZERO, amount: U256::ZERO }],
                    nonce: U256::ZERO,
                    deadline: U256::ZERO,
                },
                owner: Address::ZERO,
                signature: Signature::test_signature().as_bytes().into(),
            },
            vec![Output {
                token: Address::ZERO,
                amount: U256::ZERO,
                recipient: Address::ZERO,
                chainId: 0,
            }],
        )
    }

    #[test]
    fn test_order_hash() {
        let order = basic_order();
        let hash = order.order_hash();
        let pre_image = order.order_hash_pre_image();

        assert_eq!(hash, keccak256(pre_image));
        assert_eq!(
            hash,
            b256!("0xba359dd4f891bed0a2cf87c306e59fb6ee099e02b5b0fa86584cdcc44bf6c272")
        );
    }
}
