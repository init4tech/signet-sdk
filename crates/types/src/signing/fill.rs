use crate::agg::AggregateOrders;
use crate::signing::{permit_signing_info, SignedPermitError, SigningError};
use alloy::{primitives::Address, signers::Signer};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use signet_zenith::RollupOrders::{Output, Permit2Batch, TokenPermissions};
use std::{borrow::Cow, collections::HashMap};

/// SignedFill type is constructed by Fillers to fill a batch of Orders.
/// It represents the Orders' Outputs after they have been permit2-encoded and signed.
///
/// A SignedFill corresponds to the parameters for `fillPermit2` on the OrderDestination contract,
/// and thus contains all necessary information to fill the Order.
///
/// SignedFill is an optional part of the SignetEthBundle type.
/// Fillers sign & send bundles which contain Order initiations & fills.
/// Filler bundles contain:
/// - optionally, a host SignedFill (if any Orders contain host Outputs)
/// - optionally, a rollup transaction that submits a SignedFill (if any Orders contain rollup Outputs)
/// - rollup transactions that submit the SignedOrders
///
/// # Warning ⚠️
/// A SignedFill *must* remain private until it is mined, as there is no guarantee
/// that desired Order Inputs will be received in return for the Outputs offered by the signed Permit2Batch.
/// SignetEthBundles are used to submit SignedFills because they *must* be submitted atomically
/// with the corresponding SignedOrder(s) in order to claim the Inputs.
/// It is important to use private transaction relays to send bundles containing SignedFill(s) to Builders.
/// Bundles can be sent to a *trusted* Signet Node's `signet_sendBundle` endpoint.
///
/// TODO: Link to docs.
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

/// An UnsignedFill is a helper type used to easily transform an AggregateOrder into a single SignedFill with correct permit2 semantics.
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
        for destination_chain_id in self.orders.destination_chain_ids() {
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
        // if nonce is are None, populate it as the current timestamp in microseconds
        let nonce = self.nonce.unwrap_or(now.timestamp_micros() as u64);
        // if deadline is None, populate it as now + 12 seconds (can only mine within the current block)
        let deadline = self.deadline.unwrap_or(now.timestamp() as u64 + 12);

        // get the destination order address
        let destination_order_address = self
            .destination_chains
            .get(&chain_id)
            .ok_or(SigningError::MissingOrderContract(chain_id))?;

        // get the outputs for the chain from the AggregateOrders
        let outputs = self.orders.outputs_for(chain_id);
        // generate the permitted tokens from the Outputs
        let permitted: Vec<TokenPermissions> = outputs.iter().map(Into::into).collect();

        // generate the permit2 signing info
        let permit = permit_signing_info(
            outputs,
            permitted,
            deadline,
            nonce,
            chain_id,
            *destination_order_address,
        );

        // sign it
        let signature = signer.sign_hash(&permit.signing_hash).await?;

        // return as a SignedFill
        Ok(SignedFill {
            permit: Permit2Batch {
                permit: permit.permit,
                owner: signer.address(),
                signature: signature.as_bytes().into(),
            },
            outputs: permit.outputs,
        })
    }
}
