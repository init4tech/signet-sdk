//! Tests for the bundle guarantees.
//!
//! - Txns must not revert, unless marked as revertible.
//! - Txns must not be dropped by market rules, unless marked as droppable.

use alloy::primitives::{Address, U256};
use signet_bundle::{BundleInspector, SignetEthBundleDriver, SignetEthBundleError};
use signet_evm::EvmNeedsTx;
use signet_test_utils::{
    chain::{RU_CHAIN_ID, TEST_SYS},
    contracts::{
        counter::{Counter, COUNTER_SLOT, COUNTER_TEST_ADDRESS},
        reverts::REVERT_TEST_ADDRESS,
    },
    evm::test_signet_evm_with_inspector,
    specs::{sign_tx_with_key_pair, simple_bundle, simple_call, simple_send},
    users::TEST_SIGNERS,
};
use std::time::{Duration, Instant};
use trevm::{
    inspectors::{Layered, TimeLimit},
    revm::{database::InMemoryDB, inspector::NoOpInspector},
    BundleDriver, BundleError, NoopBlock,
};

fn bundle_evm() -> EvmNeedsTx<InMemoryDB, BundleInspector> {
    let inspector: BundleInspector<_> =
        Layered::new(TimeLimit::new(Duration::from_secs(5)), NoOpInspector);
    test_signet_evm_with_inspector(inspector).fill_block(&NoopBlock)
}

#[tokio::test]
async fn test_bundle_ok() {
    let trevm = bundle_evm();
    let recipient = Address::repeat_byte(0x31);

    // This bundle contains two transactions:
    // 1. A simple send from user 0 to 0x3131....
    // 2. A call to the Counter contract to increment the counter.
    let user_wallet = &TEST_SIGNERS[0];

    let tx_1 = simple_send(recipient, U256::ONE, 0, RU_CHAIN_ID);
    let tx_2 = simple_call(COUNTER_TEST_ADDRESS, &Counter::incrementCall, 1, RU_CHAIN_ID);

    let tx_1 = sign_tx_with_key_pair(user_wallet, tx_1);
    let tx_2 = sign_tx_with_key_pair(user_wallet, tx_2);

    let bundle = simple_bundle(&[tx_1, tx_2], None, 0);

    let mut driver = SignetEthBundleDriver::new(
        &bundle,
        TEST_SYS.host_chain_id(),
        Instant::now() + Duration::from_secs(5),
    );

    // We expect this to work.
    let trevm = driver.run_bundle(trevm).unwrap();

    // Assert that the bundle was executed successfully.

    // Check the balance of the recipient increased.
    assert_eq!(trevm.read_balance_ref(recipient), U256::ONE);
    assert_eq!(trevm.read_storage_ref(COUNTER_TEST_ADDRESS, COUNTER_SLOT), U256::from(1));
}

#[tokio::test]
async fn test_bundle_revert() {
    let trevm = bundle_evm();
    let recipient = Address::repeat_byte(0x31);

    // This bundle contains two transactions:
    // 1. A simple send from user 0 to 0x3131....
    // 2. A call to the Counter contract to increment the counter.
    let user_wallet = &TEST_SIGNERS[0];

    let tx_1 = simple_send(recipient, U256::ONE, 0, RU_CHAIN_ID);
    let tx_2 = simple_call(REVERT_TEST_ADDRESS, &Counter::incrementCall, 1, RU_CHAIN_ID);

    let tx_1 = sign_tx_with_key_pair(user_wallet, tx_1);
    let tx_2 = sign_tx_with_key_pair(user_wallet, tx_2);

    let bundle = simple_bundle(&[tx_1, tx_2], None, 0);

    let mut driver = SignetEthBundleDriver::new(
        &bundle,
        TEST_SYS.host_chain_id(),
        Instant::now() + Duration::from_secs(5),
    );

    let (err, trevm) = driver.run_bundle(trevm).unwrap_err().take_err();
    assert!(matches!(err, SignetEthBundleError::BundleError(BundleError::BundleReverted)));

    // Bundle should not have executed any transactions.
    assert_eq!(trevm.read_balance_ref(recipient), U256::ZERO);
    assert_eq!(trevm.read_storage_ref(COUNTER_TEST_ADDRESS, COUNTER_SLOT), U256::ZERO);
}

#[tokio::test]
async fn test_bundle_droppable() {
    let trevm = bundle_evm();
    let recipient = Address::repeat_byte(0x31);

    // This bundle contains two transactions:
    // 1. A simple send from user 0 to 0x3131....
    // 2. A call to the Counter contract to increment the counter.
    let user_wallet = &TEST_SIGNERS[0];

    let tx_1 = simple_send(recipient, U256::ONE, 0, RU_CHAIN_ID);
    let tx_2 = simple_call(REVERT_TEST_ADDRESS, &Counter::incrementCall, 1, RU_CHAIN_ID);

    let tx_1 = sign_tx_with_key_pair(user_wallet, tx_1);
    let tx_2 = sign_tx_with_key_pair(user_wallet, tx_2);

    // Mark the second transaction as droppable.
    let hash = *tx_2.hash();
    let mut bundle = simple_bundle(&[tx_1, tx_2], None, 0);
    bundle.bundle.reverting_tx_hashes.push(hash);

    let mut driver = SignetEthBundleDriver::new(
        &bundle,
        TEST_SYS.host_chain_id(),
        Instant::now() + Duration::from_secs(5),
    );

    // We expect this to work and drop the second transaction.
    let trevm = driver.run_bundle(trevm).unwrap();

    // Check the balance of the recipient increased, and the counter was not incremented.
    assert_eq!(trevm.read_balance_ref(recipient), U256::ONE);
    assert_eq!(trevm.read_storage_ref(COUNTER_TEST_ADDRESS, COUNTER_SLOT), U256::ZERO);
}
