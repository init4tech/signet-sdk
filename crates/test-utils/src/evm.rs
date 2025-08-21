use signet_constants::test_utils::*;
use trevm::revm::{
    context::CfgEnv, database::in_memory_db::InMemoryDB, primitives::hardfork::SpecId,
    state::Bytecode,
};

use crate::contracts::{
    counter::{COUNTER_BYTECODE, COUNTER_TEST_ADDRESS},
    system::{RU_ORDERS_BYTECODE, RU_PASSAGE_BYTECODE},
    token::{
        MINTER, MINTER_SLOT, NAME_SLOT, SYMBOL_SLOT, TOKEN_BYTECODE, WBTC_NAME, WBTC_SYMBOL,
        WETH_NAME, WETH_SYMBOL,
    },
};

/// Create a new Signet EVM with an in-memory database for testing. Deploy
/// system contracts and pre-deployed tokens.
pub fn test_signet_evm() -> signet_evm::EvmNeedsBlock<InMemoryDB> {
    let mut evm = signet_evm::signet_evm(InMemoryDB::default(), TEST_SYS).fill_cfg(&TestCfg);

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
