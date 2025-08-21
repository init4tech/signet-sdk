//! Tests for the bundle guarantees.
//!
//! - Txns must not revert, unless marked as revertible.
//! - Txns must not be dropped by market rules, unless marked as droppable.

use alloy::{
    consensus::TypedTransaction,
    primitives::{keccak256, Address, U256},
    signers::local::PrivateKeySigner,
    uint,
};
use signet_bundle::{
    BundleInspector, SignetEthBundle, SignetEthBundleDriver, SignetEthBundleError,
};
use signet_constants::pecorino::{HOST_WBTC, HOST_WETH};
use signet_evm::EvmNeedsTx;
use signet_test_utils::{
    chain::{HOST_CHAIN_ID, RU_CHAIN_ID, RU_ORDERS, TEST_SYS},
    contracts::{
        counter::{Counter, COUNTER_SLOT, COUNTER_TEST_ADDRESS},
        reverts::REVERT_TEST_ADDRESS,
    },
    evm::test_signet_evm_with_inspector,
    specs::{sign_tx_with_key_pair, simple_bundle, simple_call, simple_send},
    users::{TEST_SIGNERS, TEST_USERS},
};
use signet_types::SignedFill;
use signet_zenith::HostOrders::{
    initiateCall, Input, Output, Permit2Batch, PermitBatchTransferFrom, TokenPermissions,
};
use std::{
    sync::LazyLock,
    time::{Duration, Instant},
};
use trevm::{
    inspectors::{Layered, TimeLimit},
    revm::{database::InMemoryDB, inspector::NoOpInspector},
    BundleDriver, BundleError, NoopBlock,
};

const SENDER_WALLET: LazyLock<&PrivateKeySigner> = LazyLock::new(|| &TEST_SIGNERS[0]);

const ORDERER: LazyLock<Address> = LazyLock::new(|| TEST_USERS[1]);
const ORDERER_WALLET: LazyLock<&PrivateKeySigner> = LazyLock::new(|| &TEST_SIGNERS[1]);

const FILLER: Address = Address::repeat_byte(0x30);
const TX_0_RECIPIENT: Address = Address::repeat_byte(0x31);
const TX_2_RECIPIENT: Address = Address::repeat_byte(0x32);

const INPUT_AMOUNT: U256 = uint!(100_000_000_000_000_000_000_U256);
const OUTPUT_WBTC: U256 = uint!(100_U256);
const OUTPUT_WETH: U256 = uint!(200_U256);

fn bundle_evm() -> EvmNeedsTx<InMemoryDB, BundleInspector> {
    let inspector: BundleInspector<_> =
        Layered::new(TimeLimit::new(Duration::from_secs(5)), NoOpInspector);
    test_signet_evm_with_inspector(inspector).fill_block(&NoopBlock)
}

fn permit_2_batch(owner: Address, nonce: U256) -> Permit2Batch {
    Permit2Batch {
        permit: PermitBatchTransferFrom {
            permitted: vec![
                TokenPermissions { token: HOST_WBTC, amount: OUTPUT_WBTC },
                TokenPermissions { token: HOST_WETH, amount: OUTPUT_WETH },
            ],
            nonce,
            deadline: U256::MAX,
        },
        owner,
        signature: Default::default(),
    }
}

fn host_fills(owner: Address, nonce: U256) -> SignedFill {
    let permit = permit_2_batch(owner, nonce);
    let outputs = vec![
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
    ];
    SignedFill { permit, outputs }
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
fn test_bundle(
    counter_reverts: bool,
    order: bool,
    host_fills: Option<SignedFill>,
) -> SignetEthBundle {
    let call_addr = if counter_reverts { REVERT_TEST_ADDRESS } else { COUNTER_TEST_ADDRESS };

    let tx_1 = simple_send(TX_0_RECIPIENT, U256::ONE, 0, RU_CHAIN_ID);
    let tx_2 = if order {
        simple_order(0)
    } else {
        simple_call(call_addr, &Counter::incrementCall, U256::ZERO, 0, RU_CHAIN_ID)
    };
    let tx_3 = simple_send(TX_2_RECIPIENT, U256::ONE, 1, RU_CHAIN_ID);

    let tx_1 = sign_tx_with_key_pair(&*SENDER_WALLET, tx_1);
    let tx_2 = sign_tx_with_key_pair(&*ORDERER_WALLET, tx_2);
    let tx_3 = sign_tx_with_key_pair(&*SENDER_WALLET, tx_3);

    simple_bundle(&[tx_1, tx_2, tx_3], host_fills, 0)
}

fn order_bundle(host_fills: Option<SignedFill>) -> SignetEthBundle {
    test_bundle(false, true, host_fills)
}

fn counter_bundle(should_revert: bool) -> SignetEthBundle {
    test_bundle(should_revert, false, None)
}

#[test]
fn test_bundle_ok() {
    let trevm = bundle_evm();

    let bundle = counter_bundle(false);

    let mut driver = SignetEthBundleDriver::new(
        &bundle,
        TEST_SYS.host_chain_id(),
        Instant::now() + Duration::from_secs(5),
    );

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

    let mut driver = SignetEthBundleDriver::new(
        &bundle,
        TEST_SYS.host_chain_id(),
        Instant::now() + Duration::from_secs(5),
    );

    let (err, trevm) = driver.run_bundle(trevm).unwrap_err().take_err();
    assert!(matches!(err, SignetEthBundleError::BundleError(BundleError::BundleReverted)));

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

    let mut driver = SignetEthBundleDriver::new(
        &bundle,
        TEST_SYS.host_chain_id(),
        Instant::now() + Duration::from_secs(5),
    );

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

    let host_fills = host_fills(FILLER, U256::from(0));

    let bundle = order_bundle(Some(host_fills));

    let mut driver = SignetEthBundleDriver::new(
        &bundle,
        TEST_SYS.host_chain_id(),
        Instant::now() + Duration::from_secs(5),
    );

    // We expect this to work and drop the second transaction.
    let trevm = driver.run_bundle(trevm).unwrap();

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
    let bundle = order_bundle(None);

    let mut driver = SignetEthBundleDriver::new(
        &bundle,
        TEST_SYS.host_chain_id(),
        Instant::now() + Duration::from_secs(5),
    );

    let (err, trevm) = driver.run_bundle(trevm).unwrap_err().take_err();
    assert!(matches!(err, SignetEthBundleError::BundleError(BundleError::BundleReverted)));

    // Erroring leaves the evm in a dirty state. The first txn was executed,
    // the second was dropped, and the third was not executed.
    assert_eq!(trevm.read_balance_ref(TX_0_RECIPIENT), U256::ONE);
    assert_eq!(trevm.read_balance_ref(*ORDERER), inital_balance);
    assert_eq!(trevm.read_balance_ref(TX_2_RECIPIENT), U256::ZERO);
}

#[test]
fn test_order_bundle_droppable() {
    let trevm = bundle_evm();

    let inital_balance = trevm.read_balance_ref(*ORDERER);

    let mut bundle = order_bundle(None);

    // Mark the second transaction as droppable.
    let hash = keccak256(&bundle.txs()[1]);
    bundle.bundle.reverting_tx_hashes.push(hash);

    let mut driver = SignetEthBundleDriver::new(
        &bundle,
        TEST_SYS.host_chain_id(),
        Instant::now() + Duration::from_secs(5),
    );

    // We expect this to work and drop the second transaction.
    let trevm = driver.run_bundle(trevm).unwrap();

    // The order tx was dropped, but both sends were executed.
    assert_eq!(trevm.read_balance_ref(TX_0_RECIPIENT), U256::ONE);
    assert_eq!(trevm.read_balance_ref(*ORDERER), inital_balance);
    assert_eq!(trevm.read_balance_ref(TX_2_RECIPIENT), U256::ONE);
}
