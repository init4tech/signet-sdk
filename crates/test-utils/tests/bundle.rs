//! Tests for the bundle guarantees.
//!
//! - Txns must not revert, unless marked as revertible.
//! - Txns must not be dropped by market rules, unless marked as droppable.

use alloy::{
    consensus::{TxEnvelope, TypedTransaction},
    primitives::{keccak256, Address, U256},
    signers::local::PrivateKeySigner,
    uint,
};
use signet_bundle::{
    BundleInspector, SignetEthBundle, SignetEthBundleDriver, SignetEthBundleError,
};
use signet_constants::parmigiana::{HOST_WBTC, HOST_WETH};
use signet_evm::EvmNeedsTx;
use signet_test_utils::{
    chain::{HOST_CHAIN_ID, RU_CHAIN_ID, RU_ORDERS},
    contracts::{
        counter::{Counter, COUNTER_SLOT, COUNTER_TEST_ADDRESS},
        reverts::REVERT_TEST_ADDRESS,
    },
    evm::test_signet_evm_with_inspector,
    specs::{sign_tx_with_key_pair, simple_bundle, simple_call, simple_send},
    users::{TEST_SIGNERS, TEST_USERS},
};
use signet_types::AggregateFills;
use signet_zenith::HostOrders::{initiateCall, Filled, Input, Output};
use std::{borrow::Cow, sync::LazyLock, time::Duration};
use tokio::time::Instant;
use trevm::{
    inspectors::{Layered, TimeLimit},
    revm::{database::InMemoryDB, inspector::NoOpInspector},
    BundleDriver, BundleError, NoopBlock,
};

static SENDER_WALLET: LazyLock<&PrivateKeySigner> = LazyLock::new(|| &TEST_SIGNERS[0]);

static ORDERER: LazyLock<Address> = LazyLock::new(|| TEST_USERS[1]);
static ORDERER_WALLET: LazyLock<&PrivateKeySigner> = LazyLock::new(|| &TEST_SIGNERS[1]);

const TX_0_RECIPIENT: Address = Address::repeat_byte(0x31);
const TX_2_RECIPIENT: Address = Address::repeat_byte(0x32);

const INPUT_AMOUNT: U256 = uint!(100_000_000_000_000_000_000_U256);
const OUTPUT_WBTC: U256 = uint!(100_U256);
const OUTPUT_WETH: U256 = uint!(200_U256);

fn host_evm() -> EvmNeedsTx<InMemoryDB, NoOpInspector> {
    test_signet_evm_with_inspector(NoOpInspector).fill_block(&NoopBlock)
}

fn bundle_evm() -> EvmNeedsTx<InMemoryDB, BundleInspector> {
    let inspector: BundleInspector<_> =
        Layered::new(TimeLimit::new(Duration::from_secs(5)), NoOpInspector);
    test_signet_evm_with_inspector(inspector).fill_block(&NoopBlock)
}

fn host_fills() -> Filled {
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

// This bundle contains two transactions:
// 1. A simple send from user 0 to 0x3131....
// 2. A call to the Counter contract to increment the counter.
// 3. A simple send from user 0 to 0x3232....
fn test_bundle(counter_reverts: bool, order: bool, host_txs: Vec<TxEnvelope>) -> SignetEthBundle {
    let call_addr = if counter_reverts { REVERT_TEST_ADDRESS } else { COUNTER_TEST_ADDRESS };

    let tx_1 = simple_send(TX_0_RECIPIENT, U256::ONE, 0, RU_CHAIN_ID);
    let tx_2 = if order {
        simple_order(0)
    } else {
        simple_call(call_addr, &Counter::incrementCall, U256::ZERO, 0, RU_CHAIN_ID)
    };
    let tx_3 = simple_send(TX_2_RECIPIENT, U256::ONE, 1, RU_CHAIN_ID);

    let tx_1 = sign_tx_with_key_pair(&SENDER_WALLET, tx_1);
    let tx_2 = sign_tx_with_key_pair(&ORDERER_WALLET, tx_2);
    let tx_3 = sign_tx_with_key_pair(&SENDER_WALLET, tx_3);

    simple_bundle(vec![tx_1, tx_2, tx_3], host_txs, 0)
}

fn order_bundle(host_txs: Vec<TxEnvelope>) -> SignetEthBundle {
    test_bundle(false, true, host_txs)
}

fn counter_bundle(should_revert: bool) -> SignetEthBundle {
    test_bundle(should_revert, false, vec![])
}

#[test]
fn test_bundle_ok() {
    let trevm = bundle_evm();

    let bundle = counter_bundle(false);
    let bundle = bundle.try_to_recovered().unwrap();

    let mut driver =
        SignetEthBundleDriver::new(&bundle, host_evm(), Instant::now() + Duration::from_secs(5));

    // We expect this to work.
    let trevm = driver.run_bundle(trevm).unwrap();

    // Assert that the bundle was executed successfully.

    // Check the balance of the recipient increased.
    assert_eq!(trevm.read_balance_ref(TX_0_RECIPIENT), U256::ONE);
    assert_eq!(trevm.read_storage_ref(COUNTER_TEST_ADDRESS, COUNTER_SLOT), U256::from(1));
    assert_eq!(trevm.read_balance_ref(TX_2_RECIPIENT), U256::ONE);
}

#[test]
fn test_bundle_revert() {
    let trevm = bundle_evm();

    let bundle = counter_bundle(true);
    let bundle = bundle.try_to_recovered().unwrap();

    let mut driver =
        SignetEthBundleDriver::new(&bundle, host_evm(), Instant::now() + Duration::from_secs(5));

    let (err, trevm) = driver.run_bundle(trevm).unwrap_err().take_err();
    assert!(matches!(err, SignetEthBundleError::Bundle(BundleError::BundleReverted)));

    // Erroring leaves the evm in a dirty state. The first txn was executed,
    // the second reverted, and the third was not executed.
    assert_eq!(trevm.read_balance_ref(TX_0_RECIPIENT), U256::ONE);
    assert_eq!(trevm.read_storage_ref(COUNTER_TEST_ADDRESS, COUNTER_SLOT), U256::ZERO);
    assert_eq!(trevm.read_balance_ref(TX_2_RECIPIENT), U256::ZERO);
}

#[test]
fn test_bundle_droppable() {
    let trevm = bundle_evm();

    let mut bundle = counter_bundle(true);

    // Mark the second transaction as droppable.
    let hash = keccak256(&bundle.txs()[1]);
    bundle.bundle.reverting_tx_hashes.push(hash);

    let bundle = bundle.try_to_recovered().unwrap();
    let mut driver =
        SignetEthBundleDriver::new(&bundle, host_evm(), Instant::now() + Duration::from_secs(5));

    // We expect this to work and drop the second transaction.
    let trevm = driver.run_bundle(trevm).unwrap();

    // Check the balance of the recipients increased, and the counter was not incremented.
    assert_eq!(trevm.read_balance_ref(TX_0_RECIPIENT), U256::ONE);
    assert_eq!(trevm.read_storage_ref(COUNTER_TEST_ADDRESS, COUNTER_SLOT), U256::ZERO);
    assert_eq!(trevm.read_balance_ref(TX_2_RECIPIENT), U256::ONE);
}

#[test]
fn test_order_bundle() {
    let trevm = bundle_evm();

    let inital_balance = trevm.read_balance_ref(*ORDERER);

    let host_fills = host_fills();
    let mut agg_fills = AggregateFills::new();
    agg_fills.add_fill(HOST_CHAIN_ID, &host_fills);

    let bundle = order_bundle(vec![]);
    let bundle = bundle.try_to_recovered().unwrap();

    let mut driver = SignetEthBundleDriver::new_with_fill_state(
        &bundle,
        host_evm(),
        Instant::now() + Duration::from_secs(5),
        Cow::Owned(agg_fills),
    );

    // We expect this to work and drop the second transaction.
    let trevm = match driver.run_bundle(trevm) {
        Ok(trevm) => trevm,
        Err(err) => panic!("unexpected error running order bundle: {:?}", err.error()),
    };

    // Check the balance of the recipients increased, and the balance of the
    // sender decreased by at least the input amount.
    let post_balance = trevm.read_balance_ref(*ORDERER);

    assert_eq!(trevm.read_balance_ref(TX_0_RECIPIENT), U256::ONE);
    assert!(post_balance < inital_balance - INPUT_AMOUNT);
    assert_eq!(trevm.read_balance_ref(TX_2_RECIPIENT), U256::ONE);
}

#[test]
fn test_order_bundle_revert() {
    let trevm = bundle_evm();

    let inital_balance = trevm.read_balance_ref(*ORDERER);

    // This should cause the order to be invalid, as no fill is provided.
    let bundle = order_bundle(vec![]);
    let bundle = bundle.try_to_recovered().unwrap();

    let mut driver =
        SignetEthBundleDriver::new(&bundle, host_evm(), Instant::now() + Duration::from_secs(5));

    let (err, trevm) = driver.run_bundle(trevm).unwrap_err().take_err();
    assert!(matches!(err, SignetEthBundleError::Bundle(BundleError::BundleReverted)));

    // Erroring leaves the evm in a dirty state. The first txn was executed,
    // the second was dropped, and the third was not executed.
    assert_eq!(trevm.read_balance_ref(TX_0_RECIPIENT), U256::ONE);
    assert_eq!(trevm.read_balance_ref(*ORDERER), inital_balance);
    assert_eq!(trevm.read_balance_ref(TX_2_RECIPIENT), U256::ZERO);
}

#[test]
fn test_order_bundle_droppable() {
    tracing_subscriber::fmt::init();

    let trevm = bundle_evm();

    let inital_balance = trevm.read_balance_ref(*ORDERER);

    let mut bundle = order_bundle(vec![]);

    // Mark the second transaction as droppable.
    let hash = keccak256(&bundle.txs()[1]);
    bundle.bundle.reverting_tx_hashes.push(hash);
    dbg!(hash);

    let bundle = bundle.try_to_recovered().unwrap();
    let mut driver =
        SignetEthBundleDriver::new(&bundle, host_evm(), Instant::now() + Duration::from_secs(5));

    // We expect this to work and drop the second transaction.
    let trevm = match driver.run_bundle(trevm) {
        Ok(t) => t,
        Err(err) => panic!("unexpected error running droppable order bundle: {:?}", err.error()),
    };

    // The order tx was dropped, but both sends were executed.
    assert_eq!(trevm.read_balance_ref(TX_0_RECIPIENT), U256::ONE);
    assert_eq!(trevm.read_balance_ref(*ORDERER), inital_balance);
    assert_eq!(trevm.read_balance_ref(TX_2_RECIPIENT), U256::ONE);
}
