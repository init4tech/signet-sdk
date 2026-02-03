mod order;
pub use order::{SignedOrder, UnsignedOrder};

mod fill;
pub use fill::{SignedFill, UnsignedFill};

mod error;
pub use error::{SignedPermitError, SigningError};

use alloy::primitives::{address, Address, B256, U256};
use alloy::sol_types::{Eip712Domain, SolStruct};
use signet_zenith::RollupOrders::{
    Output, PermitBatchTransferFrom, PermitBatchWitnessTransferFrom, TokenPermissions,
};

const PERMIT2_CONTRACT_NAME: &str = "Permit2";
const PERMIT2_ADDRESS: Address = address!("0x000000000022D473030F116dDEE9F6B43aC78BA3");

/// Permit2 fields necessary for a [`SignedOrder`] or [`SignedFill`].
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::b256;
    use alloy::sol_types::SolStruct;

    /// EIP-712 signing hash test vector for TypeScript verification.
    #[derive(Debug, serde::Serialize)]
    struct Eip712TestVector {
        name: &'static str,
        /// Chain ID for the domain
        chain_id: u64,
        /// Order contract (spender) address
        order_contract: String,
        /// Token permissions
        permitted: Vec<PermittedJson>,
        /// Permit2 nonce
        nonce: String,
        /// Deadline
        deadline: String,
        /// Outputs (witness data)
        outputs: Vec<OutputJson>,
        /// Expected domain separator
        expected_domain_separator: String,
        /// Expected struct hash (type hash + encoded data)
        expected_struct_hash: String,
        /// Expected EIP-712 signing hash
        expected_signing_hash: String,
    }

    #[derive(Debug, serde::Serialize)]
    struct PermittedJson {
        token: String,
        amount: String,
    }

    #[derive(Debug, serde::Serialize)]
    struct OutputJson {
        token: String,
        amount: String,
        recipient: String,
        chain_id: u32,
    }

    fn build_eip712_test_vectors() -> Vec<Eip712TestVector> {
        vec![
            // 1. Minimal - all zeros
            {
                let permitted = vec![TokenPermissions { token: Address::ZERO, amount: U256::ZERO }];
                let outputs = vec![Output {
                    token: Address::ZERO,
                    amount: U256::ZERO,
                    recipient: Address::ZERO,
                    chainId: 0,
                }];
                let chain_id = 1u64;
                let order_contract = Address::ZERO;
                let nonce = 0u64;
                let deadline = 0u64;

                let permit_batch = PermitBatchWitnessTransferFrom {
                    permitted: permitted.clone(),
                    spender: order_contract,
                    nonce: U256::from(nonce),
                    deadline: U256::from(deadline),
                    outputs: outputs.clone(),
                };

                let domain = Eip712Domain {
                    chain_id: Some(U256::from(chain_id)),
                    name: Some(PERMIT2_CONTRACT_NAME.into()),
                    verifying_contract: Some(PERMIT2_ADDRESS),
                    version: None,
                    salt: None,
                };

                let domain_separator = domain.hash_struct();
                let struct_hash = permit_batch.eip712_hash_struct();
                let signing_hash = permit_batch.eip712_signing_hash(&domain);

                Eip712TestVector {
                    name: "minimal",
                    chain_id,
                    order_contract: format!("{:#x}", order_contract),
                    permitted: permitted
                        .iter()
                        .map(|p| PermittedJson {
                            token: format!("{:#x}", p.token),
                            amount: format!("{:#x}", p.amount),
                        })
                        .collect(),
                    nonce: format!("{:#x}", U256::from(nonce)),
                    deadline: format!("{:#x}", U256::from(deadline)),
                    outputs: outputs
                        .iter()
                        .map(|o| OutputJson {
                            token: format!("{:#x}", o.token),
                            amount: format!("{:#x}", o.amount),
                            recipient: format!("{:#x}", o.recipient),
                            chain_id: o.chainId,
                        })
                        .collect(),
                    expected_domain_separator: format!("{:#x}", domain_separator),
                    expected_struct_hash: format!("{:#x}", struct_hash),
                    expected_signing_hash: format!("{:#x}", signing_hash),
                }
            },
            // 2. Realistic order - USDC swap on mainnet
            {
                let usdc = address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
                let recipient = address!("0x1234567890123456789012345678901234567890");
                let order_contract = address!("0x96f44ddc3Bc8892371305531F1a6d8ca2331fE6C");

                let permitted =
                    vec![TokenPermissions { token: usdc, amount: U256::from(1_000_000u64) }];
                let outputs = vec![Output {
                    token: Address::ZERO,
                    amount: U256::from(500_000_000_000_000_000u64),
                    recipient,
                    chainId: 1,
                }];
                let chain_id = 1u64;
                let nonce = 12345u64;
                let deadline = 1700000000u64;

                let permit_batch = PermitBatchWitnessTransferFrom {
                    permitted: permitted.clone(),
                    spender: order_contract,
                    nonce: U256::from(nonce),
                    deadline: U256::from(deadline),
                    outputs: outputs.clone(),
                };

                let domain = Eip712Domain {
                    chain_id: Some(U256::from(chain_id)),
                    name: Some(PERMIT2_CONTRACT_NAME.into()),
                    verifying_contract: Some(PERMIT2_ADDRESS),
                    version: None,
                    salt: None,
                };

                let domain_separator = domain.hash_struct();
                let struct_hash = permit_batch.eip712_hash_struct();
                let signing_hash = permit_batch.eip712_signing_hash(&domain);

                Eip712TestVector {
                    name: "mainnet_usdc_swap",
                    chain_id,
                    order_contract: format!("{:#x}", order_contract),
                    permitted: permitted
                        .iter()
                        .map(|p| PermittedJson {
                            token: format!("{:#x}", p.token),
                            amount: format!("{:#x}", p.amount),
                        })
                        .collect(),
                    nonce: format!("{:#x}", U256::from(nonce)),
                    deadline: format!("{:#x}", U256::from(deadline)),
                    outputs: outputs
                        .iter()
                        .map(|o| OutputJson {
                            token: format!("{:#x}", o.token),
                            amount: format!("{:#x}", o.amount),
                            recipient: format!("{:#x}", o.recipient),
                            chain_id: o.chainId,
                        })
                        .collect(),
                    expected_domain_separator: format!("{:#x}", domain_separator),
                    expected_struct_hash: format!("{:#x}", struct_hash),
                    expected_signing_hash: format!("{:#x}", signing_hash),
                }
            },
            // 3. Multi-output cross-chain
            {
                let usdc = address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
                let recipient_a = address!("0x1111111111111111111111111111111111111111");
                let recipient_b = address!("0x2222222222222222222222222222222222222222");
                let order_contract = address!("0x000000000000007369676E65742D6f7264657273");

                let permitted =
                    vec![TokenPermissions { token: usdc, amount: U256::from(5_000_000u64) }];
                let outputs = vec![
                    Output {
                        token: usdc,
                        amount: U256::from(2_500_000u64),
                        recipient: recipient_a,
                        chainId: 1,
                    },
                    Output {
                        token: usdc,
                        amount: U256::from(2_500_000u64),
                        recipient: recipient_b,
                        chainId: 519,
                    },
                ];
                let chain_id = 519u64; // Signet rollup
                let nonce = 999999u64;
                let deadline = 1800000000u64;

                let permit_batch = PermitBatchWitnessTransferFrom {
                    permitted: permitted.clone(),
                    spender: order_contract,
                    nonce: U256::from(nonce),
                    deadline: U256::from(deadline),
                    outputs: outputs.clone(),
                };

                let domain = Eip712Domain {
                    chain_id: Some(U256::from(chain_id)),
                    name: Some(PERMIT2_CONTRACT_NAME.into()),
                    verifying_contract: Some(PERMIT2_ADDRESS),
                    version: None,
                    salt: None,
                };

                let domain_separator = domain.hash_struct();
                let struct_hash = permit_batch.eip712_hash_struct();
                let signing_hash = permit_batch.eip712_signing_hash(&domain);

                Eip712TestVector {
                    name: "rollup_cross_chain",
                    chain_id,
                    order_contract: format!("{:#x}", order_contract),
                    permitted: permitted
                        .iter()
                        .map(|p| PermittedJson {
                            token: format!("{:#x}", p.token),
                            amount: format!("{:#x}", p.amount),
                        })
                        .collect(),
                    nonce: format!("{:#x}", U256::from(nonce)),
                    deadline: format!("{:#x}", U256::from(deadline)),
                    outputs: outputs
                        .iter()
                        .map(|o| OutputJson {
                            token: format!("{:#x}", o.token),
                            amount: format!("{:#x}", o.amount),
                            recipient: format!("{:#x}", o.recipient),
                            chain_id: o.chainId,
                        })
                        .collect(),
                    expected_domain_separator: format!("{:#x}", domain_separator),
                    expected_struct_hash: format!("{:#x}", struct_hash),
                    expected_signing_hash: format!("{:#x}", signing_hash),
                }
            },
            // 4. Large amounts (exceeds JS safe integer)
            {
                let weth = address!("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
                let recipient = address!("0x5555555555555555555555555555555555555555");
                let order_contract = address!("0x96f44ddc3Bc8892371305531F1a6d8ca2331fE6C");

                let large_amount = U256::from(10_000_000_000_000_000_000_000u128); // 10,000 ETH

                let permitted = vec![TokenPermissions { token: weth, amount: large_amount }];
                let outputs =
                    vec![Output { token: weth, amount: large_amount, recipient, chainId: 1 }];
                let chain_id = 1u64;
                let nonce = u64::MAX;
                let deadline = u64::MAX;

                let permit_batch = PermitBatchWitnessTransferFrom {
                    permitted: permitted.clone(),
                    spender: order_contract,
                    nonce: U256::from(nonce),
                    deadline: U256::from(deadline),
                    outputs: outputs.clone(),
                };

                let domain = Eip712Domain {
                    chain_id: Some(U256::from(chain_id)),
                    name: Some(PERMIT2_CONTRACT_NAME.into()),
                    verifying_contract: Some(PERMIT2_ADDRESS),
                    version: None,
                    salt: None,
                };

                let domain_separator = domain.hash_struct();
                let struct_hash = permit_batch.eip712_hash_struct();
                let signing_hash = permit_batch.eip712_signing_hash(&domain);

                Eip712TestVector {
                    name: "large_amounts",
                    chain_id,
                    order_contract: format!("{:#x}", order_contract),
                    permitted: permitted
                        .iter()
                        .map(|p| PermittedJson {
                            token: format!("{:#x}", p.token),
                            amount: format!("{:#x}", p.amount),
                        })
                        .collect(),
                    nonce: format!("{:#x}", U256::from(nonce)),
                    deadline: format!("{:#x}", U256::from(deadline)),
                    outputs: outputs
                        .iter()
                        .map(|o| OutputJson {
                            token: format!("{:#x}", o.token),
                            amount: format!("{:#x}", o.amount),
                            recipient: format!("{:#x}", o.recipient),
                            chain_id: o.chainId,
                        })
                        .collect(),
                    expected_domain_separator: format!("{:#x}", domain_separator),
                    expected_struct_hash: format!("{:#x}", struct_hash),
                    expected_signing_hash: format!("{:#x}", signing_hash),
                }
            },
        ]
    }

    /// Test that generates EIP-712 signing hash vectors for TypeScript verification.
    /// Run with `cargo t -p signet-types eip712_signing_vectors -- --nocapture --ignored`
    #[test]
    #[ignore = "vector generation for external SDK - run manually when needed"]
    fn eip712_signing_vectors() {
        let vectors = build_eip712_test_vectors();

        println!("\n=== EIP-712 SIGNING HASH TEST VECTORS ===\n");
        println!("// Generated for TypeScript SDK verification");
        println!("// Copy this JSON array to tests/eip712-vectors.json\n");

        let output = serde_json::to_string_pretty(&vectors).unwrap();
        println!("{}", output);
        println!("\n=== END TEST VECTORS ===\n");
    }

    /// Verify minimal EIP-712 signing hash is stable.
    #[test]
    fn minimal_eip712_signing_hash() {
        let permitted = vec![TokenPermissions { token: Address::ZERO, amount: U256::ZERO }];
        let outputs = vec![Output {
            token: Address::ZERO,
            amount: U256::ZERO,
            recipient: Address::ZERO,
            chainId: 0,
        }];

        let permit_batch = PermitBatchWitnessTransferFrom {
            permitted,
            spender: Address::ZERO,
            nonce: U256::ZERO,
            deadline: U256::ZERO,
            outputs,
        };

        let domain = Eip712Domain {
            chain_id: Some(U256::from(1u64)),
            name: Some(PERMIT2_CONTRACT_NAME.into()),
            verifying_contract: Some(PERMIT2_ADDRESS),
            version: None,
            salt: None,
        };

        let signing_hash = permit_batch.eip712_signing_hash(&domain);

        // This hash must remain stable - TypeScript must produce the same value
        assert_eq!(
            signing_hash,
            b256!("0x4c8eb855427d98f29425e966e3d7526f9ed6b787a23895d12069f939fd21cc07")
        );
    }

    /// Verify domain separator computation.
    #[test]
    fn permit2_domain_separator() {
        // Mainnet domain
        let domain = Eip712Domain {
            chain_id: Some(U256::from(1u64)),
            name: Some(PERMIT2_CONTRACT_NAME.into()),
            verifying_contract: Some(PERMIT2_ADDRESS),
            version: None,
            salt: None,
        };

        let domain_separator = domain.hash_struct();

        // Print for verification - will be updated with actual value
        println!("Domain separator: {:#x}", domain_separator);

        // Known mainnet Permit2 domain separator (computed)
        assert_eq!(
            domain_separator,
            b256!("0x866a5aba21966af95d6c7ab78eb2b2fc913915c28be3b9aa07cc04ff903e3f28")
        );
    }
}
