use alloy::{
    consensus::{
        constants::GWEI_TO_WEI,
        transaction::{Recovered, SignerRecoverable},
        Signed, TxEip1559, TxEnvelope,
    },
    network::TxSigner,
    primitives::{Address, TxKind, U256},
    signers::Signature,
};
use signet_test_utils::{
    evm::test_sim_env,
    test_constants::*,
    users::{TEST_SIGNERS, TEST_USERS},
};
use std::time::Instant;

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
    let builder = test_sim_env(Instant::now() + std::time::Duration::from_millis(200));

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
#[tokio::test]
pub async fn complex_simulation() {
    let builder = test_sim_env(Instant::now() + std::time::Duration::from_millis(200));

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

    // Run the simulator
    let built = builder.build().await;

    assert_eq!(built.transactions().len(), TEST_SIGNERS.len());

    // This asserts that the builder has sorted the transactions by priority
    // fee.
    assert!(built.transactions().windows(2).all(|w| {
        let tx1 = w[0].as_eip1559().unwrap().tx().max_priority_fee_per_gas;
        let tx2 = w[1].as_eip1559().unwrap().tx().max_priority_fee_per_gas;
        tx1 >= tx2
    }));
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
