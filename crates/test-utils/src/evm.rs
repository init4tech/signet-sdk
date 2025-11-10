use std::sync::Arc;

use crate::{
    contracts::{
        counter::{COUNTER_BYTECODE, COUNTER_TEST_ADDRESS},
        reverts::{REVERT_BYTECODE, REVERT_TEST_ADDRESS},
        system::{
            HOST_ORDERS_BYTECODE, HOST_PASSAGE_BYTECODE, RU_ORDERS_BYTECODE, RU_PASSAGE_BYTECODE,
        },
        token::{allowances_slot_for, balance_slot_for, deploy_wbtc_at, deploy_weth_at},
    },
    users::TEST_USERS,
};
use alloy::{
    consensus::constants::ETH_TO_WEI,
    primitives::{Address, Bytes, KECCAK256_EMPTY, U256},
};
use signet_constants::test_utils::*;
use signet_sim::{BlockBuild, HostEnv, RollupEnv};
use trevm::{
    helpers::Ctx,
    revm::{
        context::CfgEnv,
        database::in_memory_db::InMemoryDB,
        inspector::NoOpInspector,
        primitives::hardfork::SpecId,
        state::{Account, AccountInfo, Bytecode, EvmState, EvmStorageSlot},
        Database, DatabaseCommit, Inspector,
    },
    Cfg, NoopBlock,
};

/// Create a new Signet EVM with an in-memory database for testing.
///
/// Performs initial setup to
/// - Deploy [`RU_ORDERS`] and and [`RU_PASSAGE`] system contracts
/// - Deploy a [`COUNTER`] contract for testing at [`COUNTER_TEST_ADDRESS`].
/// - Deploy Token contracts for WBTC and WETH with their respective bytecodes
///   and storage.
/// - Deploy a `Revert` contract for testing at [`REVERT_TEST_ADDRESS`].
/// - Fund the [`TEST_USERS`] with 1000 ETH each.
///
/// [`COUNTER`]: crate::contracts::counter::Counter
pub fn test_signet_evm() -> signet_evm::EvmNeedsBlock<InMemoryDB> {
    test_signet_evm_with_inspector(NoOpInspector)
}

/// Create a new Signet EVM with an in-memory database for testing.
///
/// Performs initial setup to
/// - Deploy [`RU_ORDERS`] and and [`RU_PASSAGE`] system contracts
/// - Deploy a [`COUNTER`] contract for testing at [`COUNTER_TEST_ADDRESS`].
/// - Deploy Token contracts for WBTC and WETH with their respective bytecodes
///   and storage.
/// - Deploy a `Revert` contract for testing at [`REVERT_TEST_ADDRESS`].
/// - Fund the [`TEST_USERS`] with 1000 ETH each.
///
/// [`COUNTER`]: crate::contracts::counter::Counter
pub fn test_signet_evm_with_inspector<I>(inspector: I) -> signet_evm::EvmNeedsBlock<InMemoryDB, I>
where
    I: Inspector<Ctx<InMemoryDB>>,
{
    let mut db = InMemoryDB::default();
    setup_rollup_db(&mut db).unwrap();

    signet_evm::signet_evm_with_inspector(db, inspector, TEST_SYS).fill_cfg(&TestCfg)
}

/// Test configuration for the Signet EVM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TestCfg;

impl Cfg for TestCfg {
    fn fill_cfg_env(&self, cfg_env: &mut CfgEnv) {
        let CfgEnv { chain_id, spec, .. } = cfg_env;

        *chain_id = RU_CHAIN_ID;
        *spec = SpecId::default();
    }
}

/// Test configuration for the Host EVM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostTestCfg;

impl Cfg for HostTestCfg {
    fn fill_cfg_env(&self, cfg_env: &mut CfgEnv) {
        let CfgEnv { chain_id, spec, .. } = cfg_env;

        *chain_id = HOST_CHAIN_ID;
        *spec = SpecId::default();
    }
}

/// Create a rollup EVM environment for testing the simulator
pub fn rollup_sim_env() -> RollupEnv<Arc<InMemoryDB>, NoOpInspector> {
    let mut ru_db = InMemoryDB::default();

    setup_rollup_db(&mut ru_db).unwrap();

    let ru_db = Arc::new(ru_db);

    RollupEnv::new(ru_db, TEST_SYS, &TestCfg, &NoopBlock)
}

/// Create a host EVM environment for testing.
pub fn host_sim_env() -> HostEnv<Arc<InMemoryDB>, NoOpInspector> {
    let mut host_db = InMemoryDB::default();
    setup_host_db(&mut host_db).unwrap();
    let host_db = Arc::new(host_db);

    HostEnv::new(host_db, TEST_SYS, &HostTestCfg, &NoopBlock)
}

/// Create a [`BlockBuild`] simulator environment for testing.
pub fn test_sim_env(deadline: std::time::Instant) -> BlockBuild<Arc<InMemoryDB>, Arc<InMemoryDB>> {
    let (ru_evm, host_evm) = (rollup_sim_env(), host_sim_env());
    BlockBuild::new(ru_evm, host_evm, deadline, 10, Default::default(), 50_000_000, 50_000_000)
}

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

/// Set the bytecode at the given address in the database.
fn set_bytecode_at<Db: Database + DatabaseCommit>(
    db: &mut Db,
    addr: Address,
    code: Bytes,
) -> Result<(), Db::Error> {
    modify_account(db, addr, |acct| {
        acct.set_code(Bytecode::new_legacy(code));
    })
    .map(|_| ())
    .inspect(|_| {
        assert_ne!(db.basic(addr).unwrap().unwrap().code_hash, KECCAK256_EMPTY);
    })
}

fn set_balance_of<Db: Database + DatabaseCommit>(
    db: &mut Db,
    addr: Address,
    balance: U256,
) -> Result<(), Db::Error> {
    modify_account(db, addr, |acct| {
        acct.balance = balance;
    })
    .map(|_| ())
    .inspect(|_| {
        assert_eq!(db.basic(addr).unwrap().unwrap().balance, balance);
    })
}

fn set_storage_at<Db: Database + DatabaseCommit>(
    db: &mut Db,
    addr: Address,
    slot: U256,
    value: U256,
) -> Result<(), Db::Error> {
    let mut account: Account = db.basic(addr)?.unwrap_or_default().into();
    let mut changes = EvmState::default();
    account.storage.insert(slot, EvmStorageSlot::new(value, 1));
    account.mark_touch();
    changes.insert(addr, account);
    db.commit(changes);
    assert_eq!(db.storage(addr, slot).unwrap(), value);
    Ok(())
}

fn setup_db<Db: Database + DatabaseCommit>(db: &mut Db, rollup: bool) -> Result<(), Db::Error> {
    let (weth, wbtc, orders, orders_bytecode, passage, passage_bytecode);
    if rollup {
        weth = RU_WETH;
        wbtc = RU_WBTC;
        orders = TEST_SYS.ru_orders();
        orders_bytecode = RU_ORDERS_BYTECODE;
        passage = TEST_SYS.ru_passage();
        passage_bytecode = RU_PASSAGE_BYTECODE;
    } else {
        weth = HOST_WETH;
        wbtc = HOST_WBTC;
        orders = TEST_SYS.host_orders();
        orders_bytecode = HOST_ORDERS_BYTECODE;
        passage = TEST_SYS.host_passage();
        passage_bytecode = HOST_PASSAGE_BYTECODE;
    }

    // Deploy WETH and WBTC
    deploy_weth_at(db, weth)?;
    deploy_wbtc_at(db, wbtc)?;

    // Set the bytecode for system contracts
    set_bytecode_at(db, orders, orders_bytecode)?;
    set_bytecode_at(db, passage, passage_bytecode)?;

    set_bytecode_at(db, COUNTER_TEST_ADDRESS, COUNTER_BYTECODE)?;

    // Set the bytecode for the Revert contract
    set_bytecode_at(db, REVERT_TEST_ADDRESS, REVERT_BYTECODE)?;

    let max_approve = U256::MAX;
    let token_balance = U256::from(1000 * ETH_TO_WEI);

    // increment the balance for each test signer
    TEST_USERS.iter().copied().for_each(|user| {
        set_balance_of(db, user, U256::from(1000 * ETH_TO_WEI)).unwrap();

        set_storage_at(db, weth, balance_slot_for(user), token_balance).unwrap();
        set_storage_at(db, weth, allowances_slot_for(user, orders), max_approve).unwrap();

        set_storage_at(db, wbtc, balance_slot_for(user), token_balance).unwrap();
        set_storage_at(db, wbtc, allowances_slot_for(user, orders), max_approve).unwrap();
    });

    Ok(())
}

fn setup_rollup_db<Db: Database + DatabaseCommit>(db: &mut Db) -> Result<(), Db::Error> {
    setup_db(db, true)
}

fn setup_host_db<Db: Database + DatabaseCommit>(db: &mut Db) -> Result<(), Db::Error> {
    setup_db(db, false)
}
