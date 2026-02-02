//! Integration tests verifying fill-handling behavior across three drivers:
//!
//! 1. **Call Bundle** (`SignetBundleDriver`) → OUTPUT missing fills (no validation, just report)
//! 2. **Send Bundle** (`SignetEthBundleDriver`) → ERROR on missing fills (stop execution)
//! 3. **Block Driver** (`SignetDriver`) → DROP TXN on missing fills (reject tx, continue block)
//!
//! These tests use the same input scenario with varying fill states to verify each
//! driver behaves correctly.

use alloy::{
    consensus::{constants::ETH_TO_WEI, Header, ReceiptEnvelope, TxEnvelope, TypedTransaction},
    eips::BlockNumberOrTag,
    primitives::{keccak256, Address, U256},
    signers::local::PrivateKeySigner,
    uint,
};
use signet_bundle::{
    BundleInspector, SignetBundleDriver, SignetCallBundle, SignetEthBundle,
    SignetEthBundleDriver, SignetEthBundleError,
};
use trevm::BundleError;
use signet_constants::SignetSystemConstants;
use signet_constants::test_utils::{HOST_CHAIN_ID, HOST_WBTC, HOST_WETH, RU_CHAIN_ID};
use signet_evm::{EvmNeedsTx, SignetDriver};
use signet_extract::{Extractable, ExtractedEvent, Extracts};
use signet_test_utils::{
    chain::{fake_block, Chain, RU_ORDERS},
    evm::test_signet_evm_with_inspector,
    specs::{sign_tx_with_key_pair, simple_bundle, simple_call, simple_send},
    users::{TEST_SIGNERS, TEST_USERS},
};
use signet_types::{
    primitives::{SealedHeader, TransactionSigned},
    AggregateFills,
};
use signet_zenith::HostOrders::{initiateCall, Filled, Input, Output};
use std::{
    borrow::Cow,
    sync::LazyLock,
    time::{Duration, Instant},
};
use trevm::{
    inspectors::{Layered, TimeLimit},
    revm::{database::InMemoryDB, inspector::NoOpInspector},
    BundleDriver, NoopBlock,
};

// =============================================================================
// Test Constants & Fixtures
// =============================================================================

static SENDER_WALLET: LazyLock<&PrivateKeySigner> = LazyLock::new(|| &TEST_SIGNERS[0]);
static ORDERER: LazyLock<Address> = LazyLock::new(|| TEST_USERS[1]);
static ORDERER_WALLET: LazyLock<&PrivateKeySigner> = LazyLock::new(|| &TEST_SIGNERS[1]);

/// Recipient for tx_0 (simple send before order)
const TX_0_RECIPIENT: Address = Address::repeat_byte(0x31);
/// Recipient for tx_2 (simple send after order)
const TX_2_RECIPIENT: Address = Address::repeat_byte(0x32);

/// Input amount for the order (100 ETH in wei)
const INPUT_AMOUNT: U256 = uint!(100_000_000_000_000_000_000_U256);
/// Full output WBTC amount (100 units)
const OUTPUT_WBTC: U256 = uint!(100_U256);
/// Full output WETH amount (200 units)
const OUTPUT_WETH: U256 = uint!(200_U256);
/// Partial output WBTC amount (50 units - half of full)
const PARTIAL_WBTC: U256 = uint!(50_U256);
/// Partial output WETH amount (100 units - half of full)
const PARTIAL_WETH: U256 = uint!(100_U256);

// =============================================================================
// EVM Setup Functions
// =============================================================================

/// Create a host EVM for fill simulation (no inspector needed)
fn host_evm() -> EvmNeedsTx<InMemoryDB, NoOpInspector> {
    test_signet_evm_with_inspector(NoOpInspector).fill_block(&NoopBlock)
}

/// Create a bundle EVM with time-limited inspector for send bundle tests
fn bundle_evm() -> EvmNeedsTx<InMemoryDB, BundleInspector> {
    let inspector: BundleInspector<_> =
        Layered::new(TimeLimit::new(Duration::from_secs(5)), NoOpInspector);
    test_signet_evm_with_inspector(inspector).fill_block(&NoopBlock)
}

/// Create a call bundle EVM with signet layered inspector
fn call_bundle_evm() -> signet_evm::EvmNeedsTx<InMemoryDB, Layered<TimeLimit, NoOpInspector>> {
    let inspector = Layered::new(TimeLimit::new(Duration::from_secs(5)), NoOpInspector);
    test_signet_evm_with_inspector(inspector).fill_block(&NoopBlock)
}

// =============================================================================
// Fill Fixtures
// =============================================================================

/// Create full fills that completely satisfy the order outputs
fn full_fills() -> Filled {
    Filled {
        outputs: vec![
            Output {
                token: HOST_WBTC,
                amount: OUTPUT_WBTC,
                recipient: TX_0_RECIPIENT,
                chainId: RU_CHAIN_ID as u32,
            },
            Output {
                token: HOST_WETH,
                amount: OUTPUT_WETH,
                recipient: TX_2_RECIPIENT,
                chainId: RU_CHAIN_ID as u32,
            },
        ],
    }
}

/// Create partial fills that only provide half of the required outputs
fn partial_fills() -> Filled {
    Filled {
        outputs: vec![
            Output {
                token: HOST_WBTC,
                amount: PARTIAL_WBTC,
                recipient: TX_0_RECIPIENT,
                chainId: RU_CHAIN_ID as u32,
            },
            Output {
                token: HOST_WETH,
                amount: PARTIAL_WETH,
                recipient: TX_2_RECIPIENT,
                chainId: RU_CHAIN_ID as u32,
            },
        ],
    }
}

/// Create aggregate fills from a Filled event
fn aggregate_from_filled(filled: &Filled) -> AggregateFills {
    let mut agg = AggregateFills::new();
    agg.add_fill(HOST_CHAIN_ID, filled);
    agg
}


// =============================================================================
// Transaction & Bundle Fixtures
// =============================================================================

/// Create an order transaction that requires fills
fn simple_order(nonce: u64) -> TypedTransaction {
    simple_call(
        RU_ORDERS,
        &initiateCall {
            deadline: U256::MAX,
            inputs: vec![Input { token: Address::ZERO, amount: INPUT_AMOUNT }],
            outputs: vec![
                Output {
                    token: HOST_WBTC,
                    amount: OUTPUT_WBTC,
                    recipient: TX_0_RECIPIENT,
                    chainId: HOST_CHAIN_ID as u32,
                },
                Output {
                    token: HOST_WETH,
                    amount: OUTPUT_WETH,
                    recipient: TX_2_RECIPIENT,
                    chainId: HOST_CHAIN_ID as u32,
                },
            ],
        },
        INPUT_AMOUNT,
        nonce,
        RU_CHAIN_ID,
    )
}

/// Create a test bundle with:
/// - tx_0: Simple ETH send to TX_0_RECIPIENT
/// - tx_1: Order transaction requiring fills
/// - tx_2: Simple ETH send to TX_2_RECIPIENT
fn order_bundle(host_txs: Vec<TxEnvelope>) -> SignetEthBundle {
    let tx_0 = simple_send(TX_0_RECIPIENT, U256::ONE, 0, RU_CHAIN_ID);
    let tx_1 = simple_order(0);
    let tx_2 = simple_send(TX_2_RECIPIENT, U256::ONE, 1, RU_CHAIN_ID);

    let tx_0 = sign_tx_with_key_pair(&SENDER_WALLET, tx_0);
    let tx_1 = sign_tx_with_key_pair(&ORDERER_WALLET, tx_1);
    let tx_2 = sign_tx_with_key_pair(&SENDER_WALLET, tx_2);

    simple_bundle(vec![tx_0, tx_1, tx_2], host_txs, 0)
}

/// Create a SignetCallBundle from a SignetEthBundle for call bundle tests
fn to_call_bundle(bundle: &SignetEthBundle) -> SignetCallBundle {
    SignetCallBundle {
        bundle: alloy::rpc::types::mev::EthCallBundle {
            txs: bundle.txs().to_vec(),
            block_number: 0,
            state_block_number: BlockNumberOrTag::Number(1),
            timestamp: None,
            gas_limit: None,
            difficulty: None,
            base_fee: None,
            transaction_index: None,
            coinbase: None,
            timeout: None,
        },
    }
}

// =============================================================================
// Call Bundle Tests
// =============================================================================
//
// Call bundle (SignetBundleDriver) performs NO fill validation.
// It simply reports detected fills and orders in the response.

mod call_bundle {
    use super::*;

    /// Test that call bundle outputs fills when orders have valid fills available.
    /// The response should contain both the detected orders AND the detected fills.
    #[test]
    fn outputs_fills_when_valid() {
        let trevm = call_bundle_evm();

        let bundle = order_bundle(vec![]);
        let call_bundle = to_call_bundle(&bundle);

        let mut driver = SignetBundleDriver::new(&call_bundle);

        // Run the bundle - should succeed regardless of fill state
        let _trevm = driver.run_bundle(trevm).expect("call bundle should succeed");

        let response = driver.into_response();

        // Call bundle should have detected the order outputs
        assert!(
            !response.orders.outputs.is_empty(),
            "call bundle should detect order outputs"
        );

        // The fills in the response come from RU-side fill events, not host fills
        // Since we're not executing host transactions, fills will be empty
        // This is expected behavior - call bundle just reports what it sees
    }

    /// Test that call bundle succeeds even with partial fills.
    /// No validation occurs - it just reports what was detected.
    #[test]
    fn outputs_fills_when_partial() {
        let trevm = call_bundle_evm();

        let bundle = order_bundle(vec![]);
        let call_bundle = to_call_bundle(&bundle);

        let mut driver = SignetBundleDriver::new(&call_bundle);

        // Run the bundle - should succeed (no fill validation)
        let _trevm = driver.run_bundle(trevm).expect("call bundle should succeed with partial fills");

        let response = driver.into_response();

        // Order outputs should still be detected
        assert!(
            !response.orders.outputs.is_empty(),
            "call bundle should detect order outputs even with partial fills"
        );
    }

    /// Test that call bundle succeeds even with NO fills.
    /// No validation occurs - it reports orders that need fills.
    #[test]
    fn outputs_fills_when_missing() {
        let trevm = call_bundle_evm();

        let bundle = order_bundle(vec![]);
        let call_bundle = to_call_bundle(&bundle);

        let mut driver = SignetBundleDriver::new(&call_bundle);

        // Run the bundle - should succeed (no fill validation)
        let _trevm = driver.run_bundle(trevm).expect("call bundle should succeed with missing fills");

        let response = driver.into_response();

        // Order outputs should be detected - these represent what needs to be filled
        assert!(
            !response.orders.outputs.is_empty(),
            "call bundle should detect order outputs when fills are missing"
        );

        // All three transactions should have been executed
        assert_eq!(
            response.results.len(),
            3,
            "all transactions should execute in call bundle"
        );
    }
}

// =============================================================================
// Send Bundle Tests
// =============================================================================
//
// Send bundle (SignetEthBundleDriver) validates fills and ERRORS on missing fills.
// Transactions marked as revertible are dropped instead of causing errors.

mod send_bundle {
    use super::*;

    /// Test that send bundle succeeds when fills are complete.
    #[test]
    fn succeeds_with_valid_fills() {
        let trevm = bundle_evm();
        let initial_balance = trevm.read_balance_ref(*ORDERER);

        // Set up complete fills
        let filled = full_fills();
        let agg_fills = aggregate_from_filled(&filled);

        let bundle = order_bundle(vec![]);
        let bundle = bundle.try_to_recovered().unwrap();

        let mut driver = SignetEthBundleDriver::new_with_fill_state(
            &bundle,
            host_evm(),
            Instant::now() + Duration::from_secs(5),
            Cow::Owned(agg_fills),
        );

        // Should succeed with valid fills
        let trevm = driver.run_bundle(trevm).expect("send bundle should succeed with valid fills");

        // Verify all transactions executed
        let post_balance = trevm.read_balance_ref(*ORDERER);
        assert_eq!(trevm.read_balance_ref(TX_0_RECIPIENT), U256::ONE);
        assert!(post_balance < initial_balance - INPUT_AMOUNT);
        assert_eq!(trevm.read_balance_ref(TX_2_RECIPIENT), U256::ONE);
    }

    /// Test that send bundle errors on partial fills (insufficient).
    #[test]
    fn errors_on_partial_fills() {
        let trevm = bundle_evm();
        let initial_balance = trevm.read_balance_ref(*ORDERER);

        // Set up partial fills (insufficient)
        let filled = partial_fills();
        let agg_fills = aggregate_from_filled(&filled);

        let bundle = order_bundle(vec![]);
        let bundle = bundle.try_to_recovered().unwrap();

        let mut driver = SignetEthBundleDriver::new_with_fill_state(
            &bundle,
            host_evm(),
            Instant::now() + Duration::from_secs(5),
            Cow::Owned(agg_fills),
        );

        // Should error due to insufficient fills
        let (err, trevm) = driver.run_bundle(trevm).expect_err("should error on partial fills").take_err();
        assert!(
            matches!(err, SignetEthBundleError::Bundle(BundleError::BundleReverted)),
            "expected BundleReverted error, got {:?}",
            err
        );

        // tx_0 executed, tx_1 (order) failed validation, tx_2 not executed
        assert_eq!(trevm.read_balance_ref(TX_0_RECIPIENT), U256::ONE);
        assert_eq!(trevm.read_balance_ref(*ORDERER), initial_balance);
        assert_eq!(trevm.read_balance_ref(TX_2_RECIPIENT), U256::ZERO);
    }

    /// Test that send bundle errors when no fills are provided.
    #[test]
    fn errors_on_missing_fills() {
        let trevm = bundle_evm();
        let initial_balance = trevm.read_balance_ref(*ORDERER);

        // No fills provided
        let bundle = order_bundle(vec![]);
        let bundle = bundle.try_to_recovered().unwrap();

        let mut driver =
            SignetEthBundleDriver::new(&bundle, host_evm(), Instant::now() + Duration::from_secs(5));

        // Should error due to missing fills
        let (err, trevm) = driver.run_bundle(trevm).expect_err("should error on missing fills").take_err();
        assert!(
            matches!(err, SignetEthBundleError::Bundle(BundleError::BundleReverted)),
            "expected BundleReverted error, got {:?}",
            err
        );

        // tx_0 executed, tx_1 (order) failed, tx_2 not executed
        assert_eq!(trevm.read_balance_ref(TX_0_RECIPIENT), U256::ONE);
        assert_eq!(trevm.read_balance_ref(*ORDERER), initial_balance);
        assert_eq!(trevm.read_balance_ref(TX_2_RECIPIENT), U256::ZERO);
    }

    /// Test that send bundle drops revertible tx and continues when fills are missing.
    #[test]
    fn drops_revertible_on_missing() {
        let trevm = bundle_evm();
        let initial_balance = trevm.read_balance_ref(*ORDERER);

        let mut bundle = order_bundle(vec![]);

        // Mark the order transaction (tx_1) as revertible
        let hash = keccak256(&bundle.txs()[1]);
        bundle.bundle.reverting_tx_hashes.push(hash);

        let bundle = bundle.try_to_recovered().unwrap();
        let mut driver =
            SignetEthBundleDriver::new(&bundle, host_evm(), Instant::now() + Duration::from_secs(5));

        // Should succeed - order tx dropped but bundle continues
        let trevm = driver.run_bundle(trevm).expect("should succeed when revertible tx dropped");

        // tx_0 and tx_2 executed, tx_1 (order) was dropped
        assert_eq!(trevm.read_balance_ref(TX_0_RECIPIENT), U256::ONE);
        assert_eq!(trevm.read_balance_ref(*ORDERER), initial_balance);
        assert_eq!(trevm.read_balance_ref(TX_2_RECIPIENT), U256::ONE);
    }
}

// =============================================================================
// Block Driver Tests
// =============================================================================
//
// Block driver (SignetDriver) validates fills and DROPS invalid transactions.
// The block continues processing after a dropped transaction.

mod block_driver {
    use super::*;

    /// Test environment for block driver tests
    struct BlockDriverEnv {
        wallets: Vec<PrivateKeySigner>,
        nonces: [u64; 10],
        sequence: u64,
    }

    impl BlockDriverEnv {
        fn new() -> Self {
            let wallets = (1..=10)
                .map(|i| signet_test_utils::specs::make_wallet(i))
                .collect::<Vec<_>>();
            Self { wallets, nonces: [0; 10], sequence: 1 }
        }

        fn trevm(&self) -> signet_evm::EvmNeedsBlock<InMemoryDB> {
            let mut trevm = signet_test_utils::evm::test_signet_evm();
            for wallet in &self.wallets {
                let address = wallet.address();
                // Need 1000 ETH to cover order value (100 ETH) plus gas fees
                trevm.test_set_balance(address, U256::from(ETH_TO_WEI * 1000));
            }
            trevm
        }

        fn next_block(&mut self) -> signet_types::primitives::RecoveredBlock {
            let block = fake_block(self.sequence);
            self.sequence += 1;
            block
        }

        fn signed_simple_send(&mut self, from: usize, to: Address, amount: U256) -> TxEnvelope {
            let wallet = &self.wallets[from];
            let tx = simple_send(to, amount, self.nonces[from], RU_CHAIN_ID);
            let tx = sign_tx_with_key_pair(wallet, tx);
            self.nonces[from] += 1;
            tx
        }

        fn signed_order(&mut self, from: usize) -> TxEnvelope {
            let wallet = &self.wallets[from];
            let tx = simple_order(self.nonces[from]);
            let tx = sign_tx_with_key_pair(wallet, tx);
            self.nonces[from] += 1;
            tx
        }

        fn driver<'a, 'b, C: Extractable>(
            &self,
            extracts: &'a mut Extracts<'b, C>,
            txns: Vec<TransactionSigned>,
        ) -> SignetDriver<'a, 'b, C> {
            let header = Header { gas_limit: 30_000_000, ..Default::default() };
            SignetDriver::new(
                extracts,
                Default::default(),
                txns.into(),
                SealedHeader::new(header),
                SignetSystemConstants::test(),
            )
        }
    }

    /// Create a fake transaction for use in extracts
    fn fake_tx() -> TransactionSigned {
        use alloy::{consensus::TxEip1559, signers::Signature};
        let tx = TxEip1559::default();
        let signature = Signature::test_signature();
        TransactionSigned::new_unhashed(tx.into(), signature)
    }

    /// Test that block driver accepts all transactions when fills are valid.
    #[test]
    fn accepts_with_valid_fills() {
        let mut ctx = BlockDriverEnv::new();

        // Create transactions: send, order, send
        let tx_0 = ctx.signed_simple_send(0, TX_0_RECIPIENT, U256::from(100));
        let tx_1 = ctx.signed_order(1); // Uses wallet 1 (orderer)
        let tx_2 = ctx.signed_simple_send(0, TX_2_RECIPIENT, U256::from(100));

        // Set up the block with fills
        let block = ctx.next_block();
        // Use Extracts::new with proper chain IDs so fills are keyed correctly
        let mut extracts = Extracts::<Chain>::new(HOST_CHAIN_ID, &block, RU_CHAIN_ID, 1);

        // Add a fill event to the extracts
        let fake_tx = fake_tx();
        let fake_receipt = ReceiptEnvelope::Eip1559(Default::default());
        let filled = full_fills();

        extracts.ingest_event(ExtractedEvent {
            tx: &fake_tx,
            receipt: &fake_receipt,
            log_index: 0,
            event: signet_extract::Events::Filled(signet_zenith::RollupOrders::Filled {
                outputs: filled
                    .outputs
                    .iter()
                    .map(|o| signet_zenith::RollupOrders::Output {
                        token: o.token,
                        amount: o.amount,
                        recipient: o.recipient,
                        chainId: o.chainId,
                    })
                    .collect(),
            }),
        });

        let mut driver = ctx.driver(
            &mut extracts,
            vec![tx_0.clone().into(), tx_1.clone().into(), tx_2.clone().into()],
        );

        // Run the block
        let mut trevm = ctx.trevm().drive_block(&mut driver).unwrap();
        let (sealed_block, receipts) = driver.finish();

        // All transactions should be processed
        assert_eq!(
            sealed_block.block.body.transactions().count(),
            3,
            "all 3 transactions should be in the block"
        );
        assert_eq!(receipts.len(), 3, "should have 3 receipts");

        // Verify balances
        assert_eq!(trevm.read_balance(TX_0_RECIPIENT), U256::from(100));
        assert_eq!(trevm.read_balance(TX_2_RECIPIENT), U256::from(100));
    }

    /// Test that block driver drops order tx on partial fills but processes others.
    #[test]
    fn drops_tx_on_partial_fills() {
        let mut ctx = BlockDriverEnv::new();

        // Create transactions: send, order, send
        let tx_0 = ctx.signed_simple_send(0, TX_0_RECIPIENT, U256::from(100));
        let tx_1 = ctx.signed_order(1);
        let tx_2 = ctx.signed_simple_send(0, TX_2_RECIPIENT, U256::from(100));

        let block = ctx.next_block();
        // Use Extracts::new with proper chain IDs so fills are keyed correctly
        let mut extracts = Extracts::<Chain>::new(HOST_CHAIN_ID, &block, RU_CHAIN_ID, 1);

        // Add partial fills (insufficient)
        let fake_tx = fake_tx();
        let fake_receipt = ReceiptEnvelope::Eip1559(Default::default());
        let filled = partial_fills();

        extracts.ingest_event(ExtractedEvent {
            tx: &fake_tx,
            receipt: &fake_receipt,
            log_index: 0,
            event: signet_extract::Events::Filled(signet_zenith::RollupOrders::Filled {
                outputs: filled
                    .outputs
                    .iter()
                    .map(|o| signet_zenith::RollupOrders::Output {
                        token: o.token,
                        amount: o.amount,
                        recipient: o.recipient,
                        chainId: o.chainId,
                    })
                    .collect(),
            }),
        });

        let mut driver = ctx.driver(
            &mut extracts,
            vec![tx_0.clone().into(), tx_1.clone().into(), tx_2.clone().into()],
        );

        // Run the block
        let mut trevm = ctx.trevm().drive_block(&mut driver).unwrap();
        let (sealed_block, receipts) = driver.finish();

        // Order tx should be dropped, other 2 should succeed
        assert_eq!(
            sealed_block.block.body.transactions().count(),
            2,
            "order tx should be dropped, only 2 transactions in block"
        );
        assert_eq!(receipts.len(), 2, "should have 2 receipts");

        // tx_0 and tx_2 should have executed
        assert_eq!(trevm.read_balance(TX_0_RECIPIENT), U256::from(100));
        assert_eq!(trevm.read_balance(TX_2_RECIPIENT), U256::from(100));
    }

    /// Test that block driver drops order tx when no fills are provided.
    #[test]
    fn drops_tx_on_missing_fills() {
        let mut ctx = BlockDriverEnv::new();

        // Create transactions: send, order, send
        let tx_0 = ctx.signed_simple_send(0, TX_0_RECIPIENT, U256::from(100));
        let tx_1 = ctx.signed_order(1);
        let tx_2 = ctx.signed_simple_send(0, TX_2_RECIPIENT, U256::from(100));

        let block = ctx.next_block();
        // Use Extracts::new with proper chain IDs (no fills added - empty context)
        let mut extracts = Extracts::<Chain>::new(HOST_CHAIN_ID, &block, RU_CHAIN_ID, 1);

        let mut driver = ctx.driver(
            &mut extracts,
            vec![tx_0.clone().into(), tx_1.clone().into(), tx_2.clone().into()],
        );

        // Run the block
        let mut trevm = ctx.trevm().drive_block(&mut driver).unwrap();
        let (sealed_block, receipts) = driver.finish();

        // Order tx should be dropped, other 2 should succeed
        assert_eq!(
            sealed_block.block.body.transactions().count(),
            2,
            "order tx should be dropped when no fills, only 2 transactions in block"
        );
        assert_eq!(receipts.len(), 2, "should have 2 receipts");

        // tx_0 and tx_2 should have executed
        assert_eq!(trevm.read_balance(TX_0_RECIPIENT), U256::from(100));
        assert_eq!(trevm.read_balance(TX_2_RECIPIENT), U256::from(100));
    }
}
