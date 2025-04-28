mod order;
pub use order::{SignedOrder, UnsignedOrder};

mod fill;
pub use fill::{SignedFill, UnsignedFill};

mod error;
pub use error::{SignedPermitError, SigningError};

use crate::bindings::RollupOrders::{
    Output, PermitBatchTransferFrom, PermitBatchWitnessTransferFrom, TokenPermissions,
};
use alloy::primitives::{address, Address, B256, U256};
use alloy::sol_types::{Eip712Domain, SolStruct};

const PERMIT2_CONTRACT_NAME: &str = "Permit2";
const PERMIT2_ADDRESS: Address = address!("0x000000000022D473030F116dDEE9F6B43aC78BA3");

/// Permit2 fields necessary for a SignedOrder or SignedFill.
pub(crate) struct PermitSigningInfo {
    pub outputs: Vec<Output>,
    pub signing_hash: B256,
    pub permit: PermitBatchTransferFrom,
}

/// Get the necessary fields to sign a Permit2 fill for the aggregated outputs on a given chain.
pub(crate) fn permit_signing_info(
    outputs: Vec<Output>,
    permitted: Vec<TokenPermissions>,
    deadline: u64,
    permit2_nonce: u64,
    chain_id: u64,
    order_contract: Address,
) -> PermitSigningInfo {
    // calculate the Permit2 signing hash.
    let permit_batch = PermitBatchWitnessTransferFrom {
        permitted,
        spender: order_contract,
        nonce: U256::from(permit2_nonce),
        deadline: U256::from(deadline),
        outputs,
    };

    // construct EIP-712 domain for Permit2 contract
    let domain = Eip712Domain {
        chain_id: Some(U256::from(chain_id)),
        name: Some(PERMIT2_CONTRACT_NAME.into()),
        verifying_contract: Some(PERMIT2_ADDRESS),
        version: None,
        salt: None,
    };

    // generate EIP-712 signing hash
    let signing_hash = permit_batch.eip712_signing_hash(&domain);

    // construct the Permit2 batch transfer object
    let permit = PermitBatchTransferFrom {
        permitted: permit_batch.permitted,
        nonce: U256::from(permit2_nonce),
        deadline: U256::from(deadline),
    };

    // return the FillPermitInfo
    PermitSigningInfo { outputs: permit_batch.outputs, signing_hash, permit }
}
