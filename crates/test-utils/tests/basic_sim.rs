use alloy::{
    consensus::{constants::GWEI_TO_WEI, Signed, TxEip1559, TxEnvelope},
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

#[tokio::test]
pub async fn complex_simulation() {
    let builder = test_sim_env(Instant::now() + std::time::Duration::from_millis(200));

    // Set up 10 simple sends with escalating priority fee
    for (i, sender) in TEST_SIGNERS.iter().enumerate() {
        builder.sim_items().add_tx(
            signed_send_with_mfpg(
                sender,
                TEST_USERS[i],
                U256::from(1000),
                (10 - i) as u128 * GWEI_TO_WEI as u128,
            )
            .await,
            0,
        );
    }

    // Set up the simulator
    let built = builder.build().await;

    assert!(!built.transactions().is_empty());

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

fn send_with_mfpg(to: Address, value: U256, mpfpg: u128) -> TxEip1559 {
    TxEip1559 {
        nonce: 0,
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
) -> TxEnvelope {
    let mut tx = send_with_mfpg(to, value, mpfpg);
    let res = from.sign_transaction(&mut tx).await.unwrap();

    Signed::new_unhashed(tx, res).into()
}
