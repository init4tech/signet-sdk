use crate::signing::{permit_signing_info, SignedPermitError, SigningError};
use alloy::{
    network::TransactionBuilder,
    primitives::{keccak256, Address, Bytes, B256, U256},
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
use std::{borrow::Cow, sync::OnceLock};

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
    permit: Permit2Batch,
    /// The desired outputs.
    outputs: Vec<Output>,

    #[serde(skip)]
    order_hash: OnceLock<B256>,
    #[serde(skip)]
    order_hash_pre_image: OnceLock<Bytes>,
}

impl SignedOrder {
    /// Creates a new signed order.
    pub const fn new(permit: Permit2Batch, outputs: Vec<Output>) -> Self {
        Self { permit, outputs, order_hash: OnceLock::new(), order_hash_pre_image: OnceLock::new() }
    }

    /// Get the permit batch.
    pub const fn permit(&self) -> &Permit2Batch {
        &self.permit
    }

    /// Get the outputs.
    pub fn outputs(&self) -> &[Output] {
        &self.outputs
    }

    /// Decompose the SignedOrder into its parts.
    pub fn into_parts(self) -> (Permit2Batch, Vec<Output>) {
        (self.permit, self.outputs)
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
    pub fn order_hash(&self) -> &B256 {
        self.order_hash.get_or_init(|| keccak256(self.order_hash_pre_image()))
    }

    /// Get the pre-image for the order hash.
    ///
    /// This is the raw bytes that are hashed to produce the order hash.
    #[doc(hidden)]
    pub fn order_hash_pre_image(&self) -> &Bytes {
        self.order_hash_pre_image.get_or_init(|| self.compute_order_hash_pre_image())
    }

    /// Compute the pre-image for the order hash.
    #[doc(hidden)]
    fn compute_order_hash_pre_image(&self) -> Bytes {
        // 4 * 32 bytes = 128 bytes
        let mut buf = Vec::with_capacity(128);

        buf.extend_from_slice(keccak256(self.permit.permit.abi_encode()).as_slice());
        buf.extend_from_slice(keccak256(self.permit.owner.abi_encode()).as_slice());
        buf.extend_from_slice(keccak256(self.outputs.abi_encode()).as_slice());

        // Normalize the signature.
        let signature =
            alloy::primitives::Signature::from_raw(&self.permit.signature).unwrap().normalized_s();
        buf.extend_from_slice(keccak256(signature.as_bytes()).as_slice());

        buf.into()
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
        Ok(SignedOrder::new(
            Permit2Batch {
                permit: permit.permit,
                owner: signer.address(),
                signature: signature.as_bytes().into(),
            },
            permit.outputs,
        ))
    }
}

#[cfg(test)]
mod tests {
    use alloy::primitives::{address, b256, Signature, U256};
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

        assert_eq!(hash, &keccak256(pre_image));
        assert_eq!(
            hash,
            &b256!("0xba359dd4f891bed0a2cf87c306e59fb6ee099e02b5b0fa86584cdcc44bf6c272")
        );
    }

    /// Test vector struct for TypeScript SDK verification.
    #[derive(Debug, serde::Serialize)]
    struct TestVector {
        name: &'static str,
        signed_order: SignedOrder,
        expected_order_hash: B256,
        expected_order_hash_pre_image: String,
    }

    /// Deterministic test signature for vector generation.
    /// Uses fixed r, s, v values that produce consistent results.
    fn deterministic_signature() -> Bytes {
        // Fixed signature: r=1, s=2, v=27 (normalized)
        let mut sig = [0u8; 65];
        sig[31] = 1; // r = 1 (32 bytes, big-endian)
        sig[63] = 2; // s = 2 (32 bytes, big-endian)
        sig[64] = 27; // v = 27
        sig.to_vec().into()
    }

    /// Build test vectors for TypeScript SDK verification.
    /// These vectors establish deterministic expected values that
    /// the TypeScript implementation must match.
    fn build_test_vectors() -> Vec<TestVector> {
        vec![
            // 1. Minimal order - single input/output with zero values
            {
                let order = SignedOrder::new(
                    Permit2Batch {
                        permit: PermitBatchTransferFrom {
                            permitted: vec![TokenPermissions {
                                token: Address::ZERO,
                                amount: U256::ZERO,
                            }],
                            nonce: U256::ZERO,
                            deadline: U256::ZERO,
                        },
                        owner: Address::ZERO,
                        signature: deterministic_signature(),
                    },
                    vec![Output {
                        token: Address::ZERO,
                        amount: U256::ZERO,
                        recipient: Address::ZERO,
                        chainId: 0,
                    }],
                );
                let hash = *order.order_hash();
                let pre_image = order.order_hash_pre_image().to_string();
                TestVector {
                    name: "minimal_order",
                    signed_order: order,
                    expected_order_hash: hash,
                    expected_order_hash_pre_image: pre_image,
                }
            },
            // 2. Multi-input order - 3 different token inputs
            {
                let token_a = address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"); // USDC
                let token_b = address!("0xdAC17F958D2ee523a2206206994597C13D831ec7"); // USDT
                let token_c = address!("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599"); // WBTC
                let recipient = address!("0x1234567890123456789012345678901234567890");

                let order = SignedOrder::new(
                    Permit2Batch {
                        permit: PermitBatchTransferFrom {
                            permitted: vec![
                                TokenPermissions {
                                    token: token_a,
                                    amount: U256::from(1_000_000u64), // 1 USDC
                                },
                                TokenPermissions {
                                    token: token_b,
                                    amount: U256::from(2_000_000u64), // 2 USDT
                                },
                                TokenPermissions {
                                    token: token_c,
                                    amount: U256::from(100_000_000u64), // 1 WBTC
                                },
                            ],
                            nonce: U256::from(12345u64),
                            deadline: U256::from(1700000000u64),
                        },
                        owner: recipient,
                        signature: deterministic_signature(),
                    },
                    vec![Output {
                        token: Address::ZERO,
                        amount: U256::from(1_000_000_000_000_000_000u64), // 1 ETH worth
                        recipient,
                        chainId: 1,
                    }],
                );
                let hash = *order.order_hash();
                let pre_image = order.order_hash_pre_image().to_string();
                TestVector {
                    name: "multi_input",
                    signed_order: order,
                    expected_order_hash: hash,
                    expected_order_hash_pre_image: pre_image,
                }
            },
            // 3. Multi-output order - outputs to 3 recipients on same chain
            {
                let token = address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
                let owner = address!("0xDeaDbeefdEAdbeefdEadbEEFdeadbeEFdEaDbeeF");
                let recipient_a = address!("0x1111111111111111111111111111111111111111");
                let recipient_b = address!("0x2222222222222222222222222222222222222222");
                let recipient_c = address!("0x3333333333333333333333333333333333333333");

                let order = SignedOrder::new(
                    Permit2Batch {
                        permit: PermitBatchTransferFrom {
                            permitted: vec![TokenPermissions {
                                token,
                                amount: U256::from(10_000_000u64),
                            }],
                            nonce: U256::from(99999u64),
                            deadline: U256::from(1800000000u64),
                        },
                        owner,
                        signature: deterministic_signature(),
                    },
                    vec![
                        Output {
                            token,
                            amount: U256::from(3_000_000u64),
                            recipient: recipient_a,
                            chainId: 1,
                        },
                        Output {
                            token,
                            amount: U256::from(3_000_000u64),
                            recipient: recipient_b,
                            chainId: 1,
                        },
                        Output {
                            token,
                            amount: U256::from(4_000_000u64),
                            recipient: recipient_c,
                            chainId: 1,
                        },
                    ],
                );
                let hash = *order.order_hash();
                let pre_image = order.order_hash_pre_image().to_string();
                TestVector {
                    name: "multi_output",
                    signed_order: order,
                    expected_order_hash: hash,
                    expected_order_hash_pre_image: pre_image,
                }
            },
            // 4. Cross-chain order - outputs to both host (1) and rollup (421614)
            {
                let token = address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
                let owner = address!("0xCafeBabeCafeBabeCafeBabeCafeBabeCafeBabe");
                let recipient = address!("0x4444444444444444444444444444444444444444");

                let order = SignedOrder::new(
                    Permit2Batch {
                        permit: PermitBatchTransferFrom {
                            permitted: vec![TokenPermissions {
                                token,
                                amount: U256::from(5_000_000u64),
                            }],
                            nonce: U256::from(777u64),
                            deadline: U256::from(1750000000u64),
                        },
                        owner,
                        signature: deterministic_signature(),
                    },
                    vec![
                        Output {
                            token,
                            amount: U256::from(2_500_000u64),
                            recipient,
                            chainId: 1, // Mainnet host
                        },
                        Output {
                            token,
                            amount: U256::from(2_500_000u64),
                            recipient,
                            chainId: 421614, // Arbitrum Sepolia (example rollup)
                        },
                    ],
                );
                let hash = *order.order_hash();
                let pre_image = order.order_hash_pre_image().to_string();
                TestVector {
                    name: "cross_chain",
                    signed_order: order,
                    expected_order_hash: hash,
                    expected_order_hash_pre_image: pre_image,
                }
            },
            // 5. Large amounts - U256 values exceeding JS safe integer (>2^53)
            {
                let token = address!("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"); // WETH
                let owner = address!("0x5555555555555555555555555555555555555555");
                let recipient = address!("0x6666666666666666666666666666666666666666");

                // Amount larger than JS Number.MAX_SAFE_INTEGER (2^53 - 1 = 9007199254740991)
                let large_amount = U256::from(10_000_000_000_000_000_000_000u128); // 10,000 ETH

                let order = SignedOrder::new(
                    Permit2Batch {
                        permit: PermitBatchTransferFrom {
                            permitted: vec![TokenPermissions { token, amount: large_amount }],
                            nonce: U256::from(u64::MAX), // Max u64 nonce
                            deadline: U256::from(u64::MAX), // Max u64 deadline
                        },
                        owner,
                        signature: deterministic_signature(),
                    },
                    vec![Output { token, amount: large_amount, recipient, chainId: 1 }],
                );
                let hash = *order.order_hash();
                let pre_image = order.order_hash_pre_image().to_string();
                TestVector {
                    name: "large_amounts",
                    signed_order: order,
                    expected_order_hash: hash,
                    expected_order_hash_pre_image: pre_image,
                }
            },
            // 6. Mainnet config - real mainnet addresses and chain IDs
            {
                // Real mainnet contract addresses from signet-constants
                let usdc = address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
                let host_orders = address!("0x96f44ddc3Bc8892371305531F1a6d8ca2331fE6C");
                let owner = address!("0x7777777777777777777777777777777777777777");
                let recipient = address!("0x8888888888888888888888888888888888888888");

                let order = SignedOrder::new(
                    Permit2Batch {
                        permit: PermitBatchTransferFrom {
                            permitted: vec![TokenPermissions {
                                token: usdc,
                                amount: U256::from(100_000_000u64), // 100 USDC
                            }],
                            nonce: U256::from(1000000u64),
                            deadline: U256::from(1704067200u64), // Jan 1, 2024
                        },
                        owner,
                        signature: deterministic_signature(),
                    },
                    vec![
                        Output {
                            token: host_orders, // Using orders contract as token (test)
                            amount: U256::from(50_000_000u64),
                            recipient,
                            chainId: 1, // Mainnet host
                        },
                        Output {
                            token: host_orders,
                            amount: U256::from(50_000_000u64),
                            recipient,
                            chainId: 519, // Signet mainnet rollup
                        },
                    ],
                );
                let hash = *order.order_hash();
                let pre_image = order.order_hash_pre_image().to_string();
                TestVector {
                    name: "mainnet_config",
                    signed_order: order,
                    expected_order_hash: hash,
                    expected_order_hash_pre_image: pre_image,
                }
            },
        ]
    }

    /// Test that generates and verifies all serialization vectors.
    /// Run with `cargo t -p signet-types serialization_vectors -- --nocapture --ignored`
    /// to see JSON output for TypeScript import.
    #[test]
    #[ignore = "vector generation for external SDK - run manually when needed"]
    fn serialization_vectors() {
        let vectors = build_test_vectors();

        println!("\n=== SIGNET ORDER SERIALIZATION TEST VECTORS ===\n");
        println!("// Generated for TypeScript SDK verification");
        println!("// Copy this JSON array to tests/vectors.json\n");

        let mut json_vectors = Vec::new();

        for v in &vectors {
            // Verify order hash computation
            assert_eq!(
                v.signed_order.order_hash(),
                &v.expected_order_hash,
                "Order hash mismatch for {}",
                v.name
            );

            // Build JSON representation
            let json = serde_json::json!({
                "name": v.name,
                "signedOrder": v.signed_order,
                "expectedOrderHash": format!("{:#x}", v.expected_order_hash),
                "expectedOrderHashPreImage": v.expected_order_hash_pre_image,
            });
            json_vectors.push(json);
        }

        let output = serde_json::to_string_pretty(&json_vectors).unwrap();
        println!("{}", output);
        println!("\n=== END TEST VECTORS ===\n");
    }

    /// Test that verifies the minimal order matches expected hash.
    #[test]
    fn minimal_order_hash() {
        let order = SignedOrder::new(
            Permit2Batch {
                permit: PermitBatchTransferFrom {
                    permitted: vec![TokenPermissions { token: Address::ZERO, amount: U256::ZERO }],
                    nonce: U256::ZERO,
                    deadline: U256::ZERO,
                },
                owner: Address::ZERO,
                signature: deterministic_signature(),
            },
            vec![Output {
                token: Address::ZERO,
                amount: U256::ZERO,
                recipient: Address::ZERO,
                chainId: 0,
            }],
        );

        // This hash should remain stable - TypeScript must produce the same value
        assert_eq!(
            order.order_hash(),
            &b256!("0x33ed1473731924de70307a7b458dab12b27c9354ca49d18d84511f6b84e5c956")
        );
    }

    /// Test multi-input vector hash stability.
    #[test]
    fn multi_input_hash() {
        let token_a = address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
        let token_b = address!("0xdAC17F958D2ee523a2206206994597C13D831ec7");
        let token_c = address!("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599");
        let recipient = address!("0x1234567890123456789012345678901234567890");

        let order = SignedOrder::new(
            Permit2Batch {
                permit: PermitBatchTransferFrom {
                    permitted: vec![
                        TokenPermissions { token: token_a, amount: U256::from(1_000_000u64) },
                        TokenPermissions { token: token_b, amount: U256::from(2_000_000u64) },
                        TokenPermissions { token: token_c, amount: U256::from(100_000_000u64) },
                    ],
                    nonce: U256::from(12345u64),
                    deadline: U256::from(1700000000u64),
                },
                owner: recipient,
                signature: deterministic_signature(),
            },
            vec![Output {
                token: Address::ZERO,
                amount: U256::from(1_000_000_000_000_000_000u64),
                recipient,
                chainId: 1,
            }],
        );

        assert_eq!(
            order.order_hash(),
            &b256!("0x40fb849b8e0fa7ccca85f4d69660eddd83363f575bc218f0edf81b0358658702")
        );
    }

    /// Test large amounts vector hash stability (values > JS safe integer).
    #[test]
    fn large_amounts_hash() {
        let token = address!("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
        let owner = address!("0x5555555555555555555555555555555555555555");
        let recipient = address!("0x6666666666666666666666666666666666666666");
        let large_amount = U256::from(10_000_000_000_000_000_000_000u128);

        let order = SignedOrder::new(
            Permit2Batch {
                permit: PermitBatchTransferFrom {
                    permitted: vec![TokenPermissions { token, amount: large_amount }],
                    nonce: U256::from(u64::MAX),
                    deadline: U256::from(u64::MAX),
                },
                owner,
                signature: deterministic_signature(),
            },
            vec![Output { token, amount: large_amount, recipient, chainId: 1 }],
        );

        assert_eq!(
            order.order_hash(),
            &b256!("0x7401ff93a0f4d16b66cc5a51109808f6bb29560cce8d0d3e1fce44edc8474e27")
        );
    }
}
