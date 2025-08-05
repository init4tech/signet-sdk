use signet_constants::test_utils::*;
use trevm::revm::{
    context::CfgEnv, database::in_memory_db::InMemoryDB, primitives::hardfork::SpecId,
};

/// Create a new Signet EVM with an in-memory database for testing.
pub fn test_signet_evm() -> signet_evm::EvmNeedsBlock<InMemoryDB> {
    signet_evm::signet_evm(InMemoryDB::default(), TEST_SYS).fill_cfg(&TestCfg)
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
