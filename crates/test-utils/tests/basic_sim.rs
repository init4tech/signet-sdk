use alloy::{
    consensus::{
        constants::{ETH_TO_WEI, GWEI_TO_WEI},
        Signed, TxEip1559, TxEnvelope,
    },
    network::TxSigner,
    primitives::{Address, TxKind, U256},
    signers::Signature,
};
use signet_sim::{BlockBuild, SimCache};
use signet_test_utils::{
    evm::TestCfg,
    test_constants::*,
    users::{TEST_SIGNERS, TEST_USERS},
};
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};
use trevm::{
    revm::{
        database::InMemoryDB,
        inspector::NoOpInspector,
        state::{Account, AccountInfo, EvmState},
        Database, DatabaseCommit,
    },
    NoopBlock,
};

#[tokio::test]
pub async fn test_simulator() {
    let filter = EnvFilter::from_default_env();
    let fmt = tracing_subscriber::fmt::layer().with_filter(filter);
    let registry = tracing_subscriber::registry().with(fmt);
    registry.try_init().unwrap();

    let mut db = InMemoryDB::default();

    // increment the balance for each test signer
    TEST_USERS.iter().copied().for_each(|user| {
        modify_account(&mut db, user, |acct| acct.balance = U256::from(1000 * ETH_TO_WEI)).unwrap();
    });

    let db = Arc::new(db);

    // Set up 10 simple sends with escalating priority fee
    let sim_cache = SimCache::new();
    for (i, sender) in TEST_SIGNERS.iter().enumerate() {
        sim_cache.add_tx(
            signed_simple_send(
                sender,
                TEST_USERS[i],
                U256::from(1000),
                (10 - i) as u128 * GWEI_TO_WEI as u128,
            )
            .await,
            0,
        );
    }

    let time = std::time::Instant::now();

    // Set up the simulator
    let built = BlockBuild::<_, NoOpInspector>::new(
        db,
        TEST_SYS,
        TestCfg,
        NoopBlock,
        std::time::Instant::now() + std::time::Duration::from_millis(200),
        10,
        sim_cache,
        100_000_000,
    )
    .build()
    .await;

    assert!(!built.transactions().is_empty());

    // This asserts that the builder has sorted the transactions by priority
    // fee.
    assert!(built.transactions().windows(2).all(|w| {
        let tx1 = w[0].as_eip1559().unwrap().tx().max_priority_fee_per_gas;
        let tx2 = w[1].as_eip1559().unwrap().tx().max_priority_fee_per_gas;
        tx1 >= tx2
    }));

    dbg!(time.elapsed());
}

// utilities below this point are reproduced from other places, however,
// because this test modifies the _db_ rather than the _evm_,
// we need to handle them slightly differently here.

/// Modify an account with a closure and commit the modified account.
///
/// This code is reproduced and modified from trevm
fn modify_account<Db, F>(db: &mut Db, addr: Address, f: F) -> Result<AccountInfo, Db::Error>
where
    F: FnOnce(&mut AccountInfo),
    Db: Database + DatabaseCommit,
{
    let mut acct: AccountInfo = db.basic(addr)?.unwrap_or_default();
    let old = acct.clone();
    f(&mut acct);

    let mut acct: Account = acct.into();
    acct.mark_touch();

    let changes: EvmState = [(addr, acct)].into_iter().collect();
    db.commit(changes);
    Ok(old)
}

fn simple_send(to: Address, value: U256, mpfpg: u128) -> TxEip1559 {
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

async fn signed_simple_send<S: TxSigner<Signature>>(
    from: S,
    to: Address,
    value: U256,
    mpfpg: u128,
) -> TxEnvelope {
    let mut tx = simple_send(to, value, mpfpg);
    let res = from.sign_transaction(&mut tx).await.unwrap();

    Signed::new_unhashed(tx, res).into()
}
