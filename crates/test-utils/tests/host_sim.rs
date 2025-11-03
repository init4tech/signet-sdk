use alloy::primitives::{Address, U256};
use signet_test_utils::{
    evm::test_sim_env,
    specs::{signed_simple_call, simple_bundle},
    test_constants::*,
    users::TEST_SIGNERS,
};
use signet_types::UnsignedOrder;
use signet_zenith::{HostOrders::fillCall, RollupOrders::initiateCall};
use std::time::{Duration, Instant};

#[tokio::test]
async fn test_with_host_sim() {
    let builder = test_sim_env(Instant::now() + Duration::from_millis(200));

    // Set up an order, fill pair
    let order = UnsignedOrder::default()
        .with_input(TEST_SYS.rollup().tokens().weth(), U256::from(1000))
        .with_output(
            TEST_SYS.host().tokens().weth(),
            U256::from(1000),
            Address::ZERO,
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
async fn test_with_host_sim_insufficient_fill() {
    let builder = test_sim_env(Instant::now() + Duration::from_millis(200));

    // Set up an order, fill pair
    let order = UnsignedOrder::default()
        .with_input(TEST_SYS.rollup().tokens().weth(), U256::from(1000))
        .with_output(
            TEST_SYS.host().tokens().weth(),
            U256::from(1000),
            Address::ZERO,
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

    dbg!(&block.transactions());

    assert!(block.transactions().is_empty());
}
