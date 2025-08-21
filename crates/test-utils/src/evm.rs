use crate::{
    contracts::{
        counter::{COUNTER_BYTECODE, COUNTER_TEST_ADDRESS},
        reverts::{REVERT_BYTECODE, REVERT_TEST_ADDRESS},
        system::{RU_ORDERS_BYTECODE, RU_PASSAGE_BYTECODE},
        token::{
            MINTER, MINTER_SLOT, NAME_SLOT, SYMBOL_SLOT, TOKEN_BYTECODE, WBTC_NAME, WBTC_SYMBOL,
            WETH_NAME, WETH_SYMBOL,
        },
    },
    users::TEST_USERS,
};
use alloy::{consensus::constants::ETH_TO_WEI, primitives::U256};
use signet_constants::test_utils::*;
use trevm::{
    helpers::Ctx,
    revm::{
        context::CfgEnv, database::in_memory_db::InMemoryDB, inspector::NoOpInspector,
        primitives::hardfork::SpecId, state::Bytecode, Inspector,
    },
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
