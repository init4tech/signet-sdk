use std::sync::Arc;

use crate::{
    contracts::{
        counter::{COUNTER_BYTECODE, COUNTER_TEST_ADDRESS},
        reverts::{REVERT_BYTECODE, REVERT_TEST_ADDRESS},
        system::{RU_ORDERS_BYTECODE, RU_PASSAGE_BYTECODE},
        token::{
            allowances_slot_for, balance_slot_for, deploy_wbtc_at, deploy_weth_at, MINTER,
            MINTER_SLOT, NAME_SLOT, SYMBOL_SLOT, TOKEN_BYTECODE, WBTC_NAME, WBTC_SYMBOL, WETH_NAME,
            WETH_SYMBOL,
        },
    },
    users::TEST_USERS,
};
use alloy::{
    consensus::constants::ETH_TO_WEI,
    primitives::{Address, U256},
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
    NoopBlock,
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
    let mut evm = signet_evm::signet_evm_with_inspector(InMemoryDB::default(), inspector, TEST_SYS)
        .fill_cfg(&TestCfg);

    // Set the bytecode for system contracts
    evm.set_bytecode_unchecked(TEST_SYS.ru_orders(), Bytecode::new_legacy(RU_ORDERS_BYTECODE));
    evm.set_bytecode_unchecked(TEST_SYS.ru_passage(), Bytecode::new_legacy(RU_PASSAGE_BYTECODE));

    // Set WBTC bytecode and storage
    evm.set_bytecode_unchecked(RU_WBTC, Bytecode::new_legacy(TOKEN_BYTECODE));
    evm.set_storage_unchecked(RU_WBTC, NAME_SLOT, WBTC_NAME);
    evm.set_storage_unchecked(RU_WBTC, SYMBOL_SLOT, WBTC_SYMBOL);
    evm.set_storage_unchecked(RU_WBTC, MINTER_SLOT, MINTER);

    // Set WETH bytecode and storage
    evm.set_bytecode_unchecked(RU_WETH, Bytecode::new_legacy(TOKEN_BYTECODE));
    evm.set_storage_unchecked(RU_WETH, NAME_SLOT, WETH_NAME);
    evm.set_storage_unchecked(RU_WETH, SYMBOL_SLOT, WETH_SYMBOL);
    evm.set_storage_unchecked(RU_WETH, MINTER_SLOT, MINTER);

    // Set the bytecode for the Counter contract
    evm.set_bytecode_unchecked(COUNTER_TEST_ADDRESS, Bytecode::new_legacy(COUNTER_BYTECODE));

    // Set the bytecode for the Revert contract
    evm.set_bytecode_unchecked(REVERT_TEST_ADDRESS, Bytecode::new_legacy(REVERT_BYTECODE));

    // increment the balance for each test signer
    TEST_USERS.iter().copied().for_each(|user| {
        evm.set_balance_unchecked(user, U256::from(1000 * ETH_TO_WEI));
    });

    evm
}

/// Test configuration for the Signet EVM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TestCfg;

impl trevm::Cfg for TestCfg {
    fn fill_cfg_env(&self, cfg_env: &mut CfgEnv) {
        let CfgEnv { chain_id, spec, .. } = cfg_env;

        *chain_id = RU_CHAIN_ID;
        *spec = SpecId::default();
    }
}

/// Test configuration for the Host EVM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostTestCfg;

impl trevm::Cfg for HostTestCfg {
    fn fill_cfg_env(&self, cfg_env: &mut CfgEnv) {
        let CfgEnv { chain_id, spec, .. } = cfg_env;

        *chain_id = HOST_CHAIN_ID;
        *spec = SpecId::default();
    }
}

/// Create a rollup EVM environment for testing the simulator
pub fn rollup_sim_env() -> RollupEnv<Arc<InMemoryDB>, NoOpInspector> {
    let mut ru_db = InMemoryDB::default();

    // Each test user has 1000 ETH
    TEST_USERS.iter().copied().for_each(|user| {
        modify_account(&mut ru_db, user, |acct| acct.balance = U256::from(1000 * ETH_TO_WEI))
            .unwrap();
    });

    let ru_db = Arc::new(ru_db);

    RollupEnv::new(ru_db, TEST_SYS, &TestCfg, &NoopBlock)
}

/// Create a host EVM environment for testing.
pub fn host_sim_env() -> HostEnv<Arc<InMemoryDB>, NoOpInspector> {
    let mut host_db = InMemoryDB::default();

    deploy_weth_at(&mut host_db, HOST_WETH).unwrap();
    deploy_wbtc_at(&mut host_db, HOST_WBTC).unwrap();

    // Each test user
    // - Has 1000 ETH,
    // - Has 1000 WETH
    // - Has 1000 WBTC
    // - Approves the Orders contract to spend max uint of WETH and WBTC

    TEST_USERS.iter().copied().for_each(|user| {
        modify_account(&mut host_db, user, |acct| acct.balance = U256::from(1000 * ETH_TO_WEI))
            .unwrap();

        let weth_acct: Account = host_db
            .basic(HOST_WETH)
            .unwrap()
            .map(Into::<Account>::into)
            .unwrap_or_default()
            .with_storage(
                [
                    (balance_slot_for(user), EvmStorageSlot::new(U256::from(1000 * ETH_TO_WEI), 0)),
                    (
                        allowances_slot_for(user, TEST_SYS.host_orders()),
                        EvmStorageSlot::new(U256::MAX, 0),
                    ),
                ]
                .into_iter(),
            );

        let wbtc_acct: Account = host_db
            .basic(HOST_WBTC)
            .unwrap()
            .map(Into::<Account>::into)
            .unwrap_or_default()
            .with_storage(
                [
                    (balance_slot_for(user), EvmStorageSlot::new(U256::from(1000 * ETH_TO_WEI), 0)),
                    (
                        allowances_slot_for(user, TEST_SYS.host_orders()),
                        EvmStorageSlot::new(U256::MAX, 0),
                    ),
                ]
                .into_iter(),
            );

        let mut changes: EvmState = Default::default();
        changes.insert(HOST_WETH, weth_acct);
        changes.insert(HOST_WBTC, wbtc_acct);
        host_db.commit(changes);
    });

    let host_db = Arc::new(host_db);

    HostEnv::new(host_db, TEST_SYS, &HostTestCfg, &NoopBlock)
}

/// Create a [`BlockBuild`] simulator environment for testing.
pub fn test_sim_env(deadline: std::time::Instant) -> BlockBuild<Arc<InMemoryDB>, Arc<InMemoryDB>> {
    let (ru_evm, host_evm) = (rollup_sim_env(), host_sim_env());
    BlockBuild::new(ru_evm, host_evm, deadline, 10, Default::default(), 50_000_000)
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
