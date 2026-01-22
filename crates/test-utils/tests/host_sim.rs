use alloy::primitives::U256;
use signet_test_utils::{
    contracts::{
        counter::{Counter::incrementCall, COUNTER_SLOT, COUNTER_TEST_ADDRESS},
        token::ERC20::approveCall,
    },
    evm::test_sim_env,
    specs::{signed_simple_call, simple_bundle},
    test_constants::*,
    users::{TEST_SIGNERS, TEST_USERS},
};
use signet_types::UnsignedOrder;
use signet_zenith::{HostOrders::fillCall, RollupOrders::initiateCall};
use std::time::{Duration, Instant};
use trevm::revm::DatabaseRef;

#[tokio::test]
async fn host_sim() {
    let builder = test_sim_env(Instant::now() + Duration::from_millis(200));

    // Set up an order, fill pair
    let order = UnsignedOrder::default()
        .with_input(TEST_SYS.rollup().tokens().weth(), U256::from(1000))
        .with_output(
            TEST_SYS.host().tokens().weth(),
            U256::from(1000),
            TEST_USERS[5],
            TEST_SYS.host_chain_id() as u32,
        )
        .to_order();

    let fill_outputs = order
        .outputs
        .clone()
        .into_iter()
        .map(|mut output| {
            output.chainId = TEST_SYS.host_chain_id() as u32;
            output
        })
        .collect::<Vec<_>>();

    let order_call =
        initiateCall { deadline: U256::MAX, inputs: order.inputs, outputs: order.outputs };

    let fill_call = fillCall { outputs: fill_outputs };

    // Make RU and HOST transactions
    let ru_tx = signed_simple_call(
        &TEST_SIGNERS[0],
        TEST_SYS.ru_orders(),
        &order_call,
        U256::ZERO,
        0,
        TEST_SYS.ru_chain_id(),
    );

    let host_tx = signed_simple_call(
        &TEST_SIGNERS[1],
        TEST_SYS.host_orders(),
        &fill_call,
        U256::ZERO,
        0,
        TEST_SYS.host_chain_id(),
    );

    // Make a bundle containing the two transactions
    let bundle = simple_bundle(vec![ru_tx], vec![host_tx], 0);

    builder.sim_items().add_bundle(bundle, 0).unwrap();

    let block = builder.build().await;

    assert_eq!(block.transactions().len(), 1);
    assert_eq!(block.host_transactions().len(), 1);
}

#[tokio::test]
async fn host_sim_insufficient_fill() {
    let builder = test_sim_env(Instant::now() + Duration::from_millis(200));

    // Set up an order, fill pair
    let order = UnsignedOrder::default()
        .with_input(TEST_SYS.rollup().tokens().weth(), U256::from(1000))
        .with_output(
            TEST_SYS.host().tokens().weth(),
            U256::from(1000),
            TEST_USERS[5],
            TEST_SYS.host_chain_id() as u32,
        )
        .to_order();

    let fill_outputs = order
        .outputs
        .clone()
        .into_iter()
        .map(|mut output| {
            output.chainId = TEST_SYS.host_chain_id() as u32;
            output.amount -= U256::ONE; // Make the fill insufficient
            output
        })
        .collect::<Vec<_>>();

    let order_call =
        initiateCall { deadline: U256::MAX, inputs: order.inputs, outputs: order.outputs };

    let fill_call = fillCall { outputs: fill_outputs };

    // Make RU and HOST transactions
    let ru_tx = signed_simple_call(
        &TEST_SIGNERS[0],
        TEST_SYS.ru_orders(),
        &order_call,
        U256::ZERO,
        0,
        TEST_SYS.ru_chain_id(),
    );

    let host_tx = signed_simple_call(
        &TEST_SIGNERS[1],
        TEST_SYS.host_orders(),
        &fill_call,
        U256::ZERO,
        0,
        TEST_SYS.host_chain_id(),
    );

    // Make a bundle containing the two transactions
    let bundle = simple_bundle(vec![ru_tx], vec![host_tx], 0);

    builder.sim_items().add_bundle(bundle, 0).unwrap();

    let block = builder.build().await;

    assert!(block.transactions().is_empty());
    assert!(block.host_transactions().is_empty());
}

// Currently 0-score bundles are dropped entirely. This may change in future.
#[tokio::test]
async fn host_sim_insufficient_fill_reverting_ok() {
    let builder = test_sim_env(Instant::now() + Duration::from_millis(200));

    // Set up an order, fill pair
    let order = UnsignedOrder::default()
        .with_input(TEST_SYS.rollup().tokens().weth(), U256::from(1000))
        .with_output(
            TEST_SYS.host().tokens().weth(),
            U256::from(1000),
            TEST_USERS[5],
            TEST_SYS.host_chain_id() as u32,
        )
        .to_order();

    let fill_outputs = order
        .outputs
        .clone()
        .into_iter()
        .map(|mut output| {
            output.chainId = TEST_SYS.host_chain_id() as u32;
            output.amount -= U256::ONE; // Make the fill insufficient
            output
        })
        .collect::<Vec<_>>();

    let order_call =
        initiateCall { deadline: U256::MAX, inputs: order.inputs, outputs: order.outputs };

    let fill_call = fillCall { outputs: fill_outputs };

    // Make RU and HOST transactions
    let ru_tx = signed_simple_call(
        &TEST_SIGNERS[0],
        TEST_SYS.ru_orders(),
        &order_call,
        U256::ZERO,
        0,
        TEST_SYS.ru_chain_id(),
    );

    let host_tx = signed_simple_call(
        &TEST_SIGNERS[1],
        TEST_SYS.host_orders(),
        &fill_call,
        U256::ZERO,
        0,
        TEST_SYS.host_chain_id(),
    );

    // Make a bundle containing the two transactions
    let ru_tx_hash = *ru_tx.hash();
    let mut bundle = simple_bundle(vec![ru_tx], vec![host_tx], 0);
    bundle.bundle.reverting_tx_hashes.push(ru_tx_hash);

    builder.sim_items().add_bundle(bundle, 0).unwrap();

    let block = builder.build().await;

    // Score 0 bundles are dropped
    assert!(block.transactions().is_empty());
    assert!(block.host_transactions().is_empty());
}

#[tokio::test]
async fn too_much_host_gas() {
    let mut builder = test_sim_env(Instant::now() + Duration::from_millis(200));
    builder.set_max_host_gas(1); // Set max host gas very low

    // Set up an order, fill pair
    let order = UnsignedOrder::default()
        .with_input(TEST_SYS.rollup().tokens().weth(), U256::from(1000))
        .with_output(
            TEST_SYS.host().tokens().weth(),
            U256::from(1000),
            TEST_USERS[5],
            TEST_SYS.host_chain_id() as u32,
        )
        .to_order();

    let fill_outputs = order
        .outputs
        .clone()
        .into_iter()
        .map(|mut output| {
            output.chainId = TEST_SYS.host_chain_id() as u32;
            output
        })
        .collect::<Vec<_>>();

    let order_call =
        initiateCall { deadline: U256::MAX, inputs: order.inputs, outputs: order.outputs };

    let fill_call = fillCall { outputs: fill_outputs };

    // Make RU and HOST transactions
    let ru_tx = signed_simple_call(
        &TEST_SIGNERS[0],
        TEST_SYS.ru_orders(),
        &order_call,
        U256::ZERO,
        0,
        TEST_SYS.ru_chain_id(),
    );

    let host_tx = signed_simple_call(
        &TEST_SIGNERS[1],
        TEST_SYS.host_orders(),
        &fill_call,
        U256::ZERO,
        0,
        TEST_SYS.host_chain_id(),
    );

    // Make a bundle containing the two transactions
    let ru_tx_hash = *ru_tx.hash();
    let mut bundle = simple_bundle(vec![ru_tx], vec![host_tx], 0);
    bundle.bundle.reverting_tx_hashes.push(ru_tx_hash);

    builder.sim_items().add_bundle(bundle, 0).unwrap();

    let block = builder.build().await;

    assert!(block.transactions().is_empty());
    assert!(block.host_transactions().is_empty());
}

#[tokio::test]
async fn larger_bundle() {
    let builder = test_sim_env(Instant::now() + Duration::from_millis(200));

    // Set up an order, fill pair
    let order = UnsignedOrder::default()
        .with_input(TEST_SYS.rollup().tokens().weth(), U256::from(1000))
        .with_output(
            TEST_SYS.host().tokens().weth(),
            U256::from(1000),
            TEST_USERS[5],
            TEST_SYS.host_chain_id() as u32,
        )
        .to_order();

    let fill_outputs = order
        .outputs
        .clone()
        .into_iter()
        .map(|mut output| {
            output.chainId = TEST_SYS.host_chain_id() as u32;
            output
        })
        .collect::<Vec<_>>();

    let order_call =
        initiateCall { deadline: U256::MAX, inputs: order.inputs, outputs: order.outputs };

    let fill_call = fillCall { outputs: fill_outputs };

    // Make RU and HOST transactions
    let ru_tx = signed_simple_call(
        &TEST_SIGNERS[0],
        TEST_SYS.ru_orders(),
        &order_call,
        U256::ZERO,
        0,
        TEST_SYS.ru_chain_id(),
    );
    let ru_tx_1 = signed_simple_call(
        &TEST_SIGNERS[0],
        TEST_SYS.ru_orders(),
        &order_call,
        U256::ZERO,
        1,
        TEST_SYS.ru_chain_id(),
    );
    let ru_tx_2 = signed_simple_call(
        &TEST_SIGNERS[0],
        TEST_SYS.ru_orders(),
        &order_call,
        U256::ZERO,
        2,
        TEST_SYS.ru_chain_id(),
    );

    let host_tx = signed_simple_call(
        &TEST_SIGNERS[1],
        TEST_SYS.host_orders(),
        &fill_call,
        U256::ZERO,
        0,
        TEST_SYS.host_chain_id(),
    );
    let host_tx_1 = signed_simple_call(
        &TEST_SIGNERS[1],
        TEST_SYS.host_orders(),
        &fill_call,
        U256::ZERO,
        1,
        TEST_SYS.host_chain_id(),
    );
    let host_tx_2 = signed_simple_call(
        &TEST_SIGNERS[1],
        TEST_SYS.host_orders(),
        &fill_call,
        U256::ZERO,
        2,
        TEST_SYS.host_chain_id(),
    );

    let bundle =
        simple_bundle(vec![ru_tx, ru_tx_1, ru_tx_2], vec![host_tx, host_tx_1, host_tx_2], 0);

    builder.sim_items().add_bundle(bundle, 0).unwrap();

    let block = builder.build().await;

    assert_eq!(block.transactions().len(), 3);
    assert_eq!(block.host_transactions().len(), 3);
}

#[tokio::test]
async fn larger_bundle_revert_ok() {
    let builder = test_sim_env(Instant::now() + Duration::from_millis(200));

    // Set up an order, fill pair
    let order = UnsignedOrder::default()
        .with_input(TEST_SYS.rollup().tokens().weth(), U256::from(1000))
        .with_output(
            TEST_SYS.host().tokens().weth(),
            U256::from(1000),
            TEST_USERS[5],
            TEST_SYS.host_chain_id() as u32,
        )
        .to_order();

    let fill_outputs = order
        .outputs
        .clone()
        .into_iter()
        .map(|mut output| {
            output.chainId = TEST_SYS.host_chain_id() as u32;
            output
        })
        .collect::<Vec<_>>();

    let order_call =
        initiateCall { deadline: U256::MAX, inputs: order.inputs, outputs: order.outputs };

    let fill_call = fillCall { outputs: fill_outputs };

    // Make RU and HOST transactions
    let ru_tx = signed_simple_call(
        &TEST_SIGNERS[0],
        TEST_SYS.ru_orders(),
        &order_call,
        U256::ZERO,
        0,
        TEST_SYS.ru_chain_id(),
    );
    let ru_tx_1 = signed_simple_call(
        &TEST_SIGNERS[0],
        TEST_SYS.ru_orders(),
        &order_call,
        U256::ZERO,
        1,
        TEST_SYS.ru_chain_id(),
    );
    let ru_tx_2 = signed_simple_call(
        &TEST_SIGNERS[0],
        TEST_SYS.ru_orders(),
        &order_call,
        U256::ZERO,
        2,
        TEST_SYS.ru_chain_id(),
    );

    let host_tx = signed_simple_call(
        &TEST_SIGNERS[1],
        TEST_SYS.host_orders(),
        &fill_call,
        U256::ZERO,
        0,
        TEST_SYS.host_chain_id(),
    );
    let host_tx_1 = signed_simple_call(
        &TEST_SIGNERS[1],
        TEST_SYS.host_orders(),
        &fill_call,
        U256::ZERO,
        1,
        TEST_SYS.host_chain_id(),
    );

    let ru_tx_2_hash = *ru_tx_2.hash();

    let mut bundle = simple_bundle(vec![ru_tx, ru_tx_1], vec![host_tx, host_tx_1], 0);

    bundle.bundle.reverting_tx_hashes.push(ru_tx_2_hash);

    builder.sim_items().add_bundle(bundle, 0).unwrap();

    let block = builder.build().await;

    assert_eq!(block.transactions().len(), 2);
    assert_eq!(block.host_transactions().len(), 2);
}

// Test checks that host cache simulation works when consuming all host balance
#[tokio::test]
async fn host_cache_between_bundles() {
    let builder = test_sim_env(Instant::now() + Duration::from_millis(200));

    let increment_call = incrementCall {};

    // Make RU and HOST transactions
    // In this test the ru txns are dummies. We're testing that the host cache
    // updates correctly.
    let ru_tx = signed_simple_call(
        &TEST_SIGNERS[0],
        TEST_USERS[1],
        &increment_call,
        U256::ZERO,
        0,
        TEST_SYS.ru_chain_id(),
    );
    let ru_tx_1 = signed_simple_call(
        &TEST_SIGNERS[1],
        TEST_USERS[1],
        &increment_call,
        U256::ZERO,
        0,
        TEST_SYS.ru_chain_id(),
    );

    let host_tx = signed_simple_call(
        &TEST_SIGNERS[2],
        COUNTER_TEST_ADDRESS,
        &increment_call,
        U256::ZERO,
        0,
        TEST_SYS.host_chain_id(),
    );
    let host_tx_1 = signed_simple_call(
        &TEST_SIGNERS[3],
        COUNTER_TEST_ADDRESS,
        &increment_call,
        U256::ZERO,
        0,
        TEST_SYS.host_chain_id(),
    );

    let bundle = simple_bundle(vec![ru_tx], vec![host_tx], 0);
    let bundle_1 = simple_bundle(vec![ru_tx_1], vec![host_tx_1], 0);

    builder.sim_items().add_bundle(bundle, 0).unwrap();
    builder.sim_items().add_bundle(bundle_1, 0).unwrap();

    let builder = builder.run_build().await;

    assert_eq!(
        builder.host_env().db().storage_ref(COUNTER_TEST_ADDRESS, COUNTER_SLOT).unwrap(),
        U256::from(2)
    );

    let block = builder.into_block();
    assert_eq!(block.transactions().len(), 2);
    assert_eq!(block.host_transactions().len(), 2);
}

/// Test for ConcurrentDB state isolation bug.
///
/// This test validates that bundles with multiple host transactions touching
/// overlapping contract state (same ERC-20) work correctly. The bug causes
/// the second host transaction to read stale state from the cache after
/// `accept_state()` is called on the first transaction.
///
/// Pattern that should work but fails with the bug:
/// - Host TX 1: approve(spender, amount) on WETH
/// - Host TX 2: fill() which calls transferFrom on WETH
/// - Rollup TX: order initiation
///
/// With the bug, the fill transaction reads stale allowance state and fails
/// with TRANSFER_FROM_FAILED because it doesn't see the approval from TX 1.
#[tokio::test]
async fn multi_host_tx_overlapping_state_isolation_bug() {
    let builder = test_sim_env(Instant::now() + Duration::from_millis(200));

    // Set up an order that will be filled
    let order = UnsignedOrder::default()
        .with_input(TEST_SYS.rollup().tokens().weth(), U256::from(1000))
        .with_output(
            TEST_SYS.host().tokens().weth(),
            U256::from(1000),
            TEST_USERS[5],
            TEST_SYS.host_chain_id() as u32,
        )
        .to_order();

    let fill_outputs = order
        .outputs
        .clone()
        .into_iter()
        .map(|mut output| {
            output.chainId = TEST_SYS.host_chain_id() as u32;
            output
        })
        .collect::<Vec<_>>();

    let order_call =
        initiateCall { deadline: U256::MAX, inputs: order.inputs, outputs: order.outputs };

    let fill_call = fillCall { outputs: fill_outputs };

    // Host TX 1: Approve the host orders contract to spend WETH
    // This touches the WETH contract's allowance storage slot for TEST_SIGNERS[1]
    // Even though the user already has max allowance, this tx still writes to
    // the allowance storage slot and exercises the state transition path.
    let host_approve_tx = signed_simple_call(
        &TEST_SIGNERS[1],
        TEST_SYS.host().tokens().weth(), // WETH token address
        &approveCall { spender: TEST_SYS.host_orders(), amount: U256::MAX },
        U256::ZERO,
        0, // nonce 0
        TEST_SYS.host_chain_id(),
    );

    // Host TX 2: Fill the order (calls transferFrom on WETH)
    // This also touches the WETH contract:
    // - Reads allowance (must see state from TX 1 or pre-existing)
    // - Writes to balance storage slots
    // With the cache bug, this transaction may read stale state.
    let host_fill_tx = signed_simple_call(
        &TEST_SIGNERS[1],
        TEST_SYS.host_orders(),
        &fill_call,
        U256::ZERO,
        1, // nonce 1
        TEST_SYS.host_chain_id(),
    );

    // Rollup TX: Initiate the order
    let ru_tx = signed_simple_call(
        &TEST_SIGNERS[0],
        TEST_SYS.ru_orders(),
        &order_call,
        U256::ZERO,
        0,
        TEST_SYS.ru_chain_id(),
    );

    // Create bundle with:
    // - 2 host transactions touching the same ERC-20 (approve + fill)
    // - 1 rollup transaction
    let bundle = simple_bundle(vec![ru_tx], vec![host_approve_tx, host_fill_tx], 0);

    builder.sim_items().add_bundle(bundle, 0).unwrap();

    let block = builder.build().await;

    // With the ConcurrentDB bug, this fails because:
    // 1. host_approve_tx executes and modifies WETH contract state
    // 2. accept_state() is called, but cache is not properly isolated
    // 3. host_fill_tx executes and reads STALE state from cache
    // 4. transferFrom fails because it doesn't see correct allowance/balance
    //
    // After the fix, both host transactions should succeed because
    // the second transaction correctly sees the state changes from the first.
    assert_eq!(block.transactions().len(), 1, "Rollup transaction should be included");
    assert_eq!(
        block.host_transactions().len(),
        2,
        "Both host transactions (approve + fill) should succeed when state is properly isolated"
    );
}
