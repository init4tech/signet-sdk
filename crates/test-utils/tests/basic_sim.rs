use alloy::{
    consensus::{
        constants::GWEI_TO_WEI,
        transaction::{Recovered, SignerRecoverable},
        Signed, Transaction, TxEip1559, TxEnvelope,
    },
    eips::eip2718::Encodable2718,
    network::TxSigner,
    primitives::{Address, TxKind, U256},
    rpc::types::mev::EthSendBundle,
    signers::Signature,
};
use signet_bundle::SignetEthBundle;
use signet_test_utils::{
    evm::test_sim_env,
    test_constants::*,
    users::{TEST_SIGNERS, TEST_USERS},
};
use tokio::time::{Duration, Instant};

/// Tests the case where multiple transactions from the same
/// sender with successive nonces are included in the same
/// simulation batch.
///
/// It'll set up 2 transactions from the same sender with
/// the same nonce, and then 2 more transactions with the next
/// two nonces. It will then verify that the simulation
/// produces a block with 3 successful transactions.
#[tokio::test]
pub async fn successive_nonces() {
    let builder = test_sim_env(Instant::now() + Duration::from_millis(200));

    // Set up 4 sends from the same sender with successive nonces
    let sender = &TEST_SIGNERS[0];
    let to = TEST_USERS[1];

    for nonce in 0..4u64 {
        let tx = signed_send_with_mfpg(
            sender,
            to,
            U256::from(1000),
            GWEI_TO_WEI as u128 * 10,
            nonce.saturating_sub(1), // cute little way to duplicate nonce 0
        )
        .await;
        builder.sim_items().add_tx(tx, 0);
    }

    // Run the simulator
    let built = builder.build().await;

    assert_eq!(built.transactions().len(), 3);

    // This asserts that the builder has sorted the transactions by nonce
    assert!(built.transactions().windows(2).all(|w| {
        let tx1 = w[0].as_eip1559().unwrap().tx().nonce;
        let tx2 = w[1].as_eip1559().unwrap().tx().nonce;
        tx1 < tx2
    }));
}

/// This test simulates a transaction from each of the test signers,
/// with escalating priority fees, and asserts that the simulation
/// orders them correctly by priority fee.
#[tokio::test(start_paused = true)]
pub async fn complex_simulation() {
    let timeout = Duration::from_secs(10);
    let builder = test_sim_env(Instant::now() + timeout);

    // Set up 10 simple sends with escalating priority fee
    for (i, sender) in TEST_SIGNERS.iter().enumerate() {
        let tx = signed_send_with_mfpg(
            sender,
            TEST_USERS[i],
            U256::from(1000),
            (10 - i) as u128 * GWEI_TO_WEI as u128,
            0,
        )
        .await;
        builder.sim_items().add_tx(tx, 0);
    }

    let cache = builder.sim_items().clone();
    // Run the simulator in a separate task
    let build_task = tokio::spawn(async move { builder.build().await });

    // Wait until all 10 items have been simulated (cache becomes empty)
    let wait_for_empty_cache = async {
        loop {
            tokio::task::yield_now().await;
            if cache.is_empty() {
                break;
            }
            // We shouldn't need to manually advance time since the sleeps in
            // `BlockBuild::run_build` cause the paused time to auto-advance when there is no work
            // to be done. However, in case some unforeseen task causes this to not happen, we'll
            // manually advance a little, so that `tokio::time::timeout` will eventually time out.
            tokio::time::advance(Duration::from_micros(1)).await;
        }
    };
    tokio::time::timeout(timeout, wait_for_empty_cache)
        .await
        .expect("timed out waiting for empty cache");

    // Advance time past the deadline to complete the build
    tokio::time::advance(timeout).await;
    let built = build_task.await.unwrap();

    // All 10 transactions should be included since we waited for all to process
    assert_eq!(built.transactions().len(), 10);

    // This asserts that the builder has sorted the transactions by priority
    // fee.
    assert!(built.transactions().windows(2).all(|w| {
        let tx1 = w[0].as_eip1559().unwrap().tx().max_priority_fee_per_gas;
        let tx2 = w[1].as_eip1559().unwrap().tx().max_priority_fee_per_gas;
        tx1 >= tx2
    }));
}

/// Test the siulator correctly handles bundle future validity.
/// This will make a bundle, with 2 txs from different senders. One tx will
/// have nonce 0, while the other will have nonce 1. We will also ingest a tx
/// from the second sender with nonce 0 into the simcache.
///
/// The simulator should output a block containing all 3 transactions. First
/// the solo tx, then the bundle txns.
#[tokio::test]
async fn test_bundle_future_validity() {
    signet_test_utils::init_tracing();

    let builder = test_sim_env(Instant::now() + Duration::from_millis(200));

    let sender_0 = &TEST_SIGNERS[0];
    let sender_1 = &TEST_SIGNERS[1];

    let to = TEST_USERS[2];

    let bare_tx =
        signed_send_with_mfpg(sender_0, to, U256::from(1000), GWEI_TO_WEI as u128 * 10, 0).await;
    let bundle_tx_0 =
        signed_send_with_mfpg(sender_0, to, U256::from(1000), GWEI_TO_WEI as u128 * 10, 1)
            .await
            .encoded_2718()
            .into();
    let bundle_tx_1 =
        signed_send_with_mfpg(sender_1, to, U256::from(1000), GWEI_TO_WEI as u128 * 10, 0)
            .await
            .encoded_2718()
            .into();

    let bundle = SignetEthBundle {
        bundle: EthSendBundle {
            txs: vec![bundle_tx_0, bundle_tx_1],
            replacement_uuid: Some(Default::default()),
            ..Default::default()
        },
        host_txs: vec![],
    };

    // Add the bundle and bare tx to the simulator
    builder.sim_items().add_bundle(bundle, 0).unwrap();

    // Run the simulator
    let cache = builder.sim_items().clone();
    let build_task = tokio::spawn(async move { builder.build().await });

    // We will inject the bare tx after a short delay to ensure
    // it is added during the simulation. This checks that the bundle is
    // simulated as "Validity::Future" at least once before the tx is added.
    tokio::time::sleep(Duration::from_millis(50)).await;
    cache.add_tx(bare_tx, 0);

    let built = build_task.await.unwrap();

    assert_eq!(built.transactions().len(), 3);
    assert_eq!(built.transactions()[0].nonce(), 0);
    // Bundle order is preserved
    assert_eq!(built.transactions()[1].nonce(), 1);
    assert_eq!(built.transactions()[2].nonce(), 0);
    assert_eq!(built.transactions()[0].signer(), built.transactions()[1].signer());
}

// utilities below this point are reproduced from other places, however,
// because this test modifies the _db_ rather than the _evm_,
// we need to handle them slightly differently here.

/// Modify an account with a closure and commit the modified account.
///
/// This code is reproduced and modified from trevm
fn send_with_mfpg(to: Address, value: U256, mpfpg: u128, nonce: u64) -> TxEip1559 {
    TxEip1559 {
        nonce,
        gas_limit: 21_000,
        to: TxKind::Call(to),
        value,
        chain_id: RU_CHAIN_ID,
        max_fee_per_gas: GWEI_TO_WEI as u128 * 100,
        max_priority_fee_per_gas: mpfpg,
        ..Default::default()
    }
}

async fn signed_send_with_mfpg<S: TxSigner<Signature>>(
    from: S,
    to: Address,
    value: U256,
    mpfpg: u128,
    nonce: u64,
) -> Recovered<TxEnvelope> {
    let mut tx = send_with_mfpg(to, value, mpfpg, nonce);
    let res = from.sign_transaction(&mut tx).await.unwrap();

    TxEnvelope::from(Signed::new_unhashed(tx, res)).try_into_recovered().unwrap()
}
