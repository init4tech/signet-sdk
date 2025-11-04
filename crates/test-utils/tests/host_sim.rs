use alloy::primitives::U256;
use signet_test_utils::{
    evm::test_sim_env,
    specs::{signed_simple_call, simple_bundle},
    test_constants::*,
    users::{TEST_SIGNERS, TEST_USERS},
};
use signet_types::UnsignedOrder;
use signet_zenith::{HostOrders::fillCall, RollupOrders::initiateCall};
use std::time::{Duration, Instant};

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
