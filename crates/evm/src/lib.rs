#![warn(
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    unreachable_pub,
    clippy::missing_const_for_fn,
    rustdoc::all
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![deny(unused_must_use, rust_2018_idioms)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

//! Signet EVM

mod aliases;
pub use aliases::*;

/// Utilities for converting types to Reth primitives.
pub mod convert;
pub use convert::ToRethPrimitive;

mod driver;
pub use driver::SignetDriver;

mod journal;
pub use journal::HostJournal;

mod orders;
pub use orders::{Framed, FramedFilleds, FramedOrders, OrderDetector, SignetInspector};

mod result;
pub use result::BlockResult;

use signet_types::config::SignetSystemConstants;
use trevm::{
    helpers::Ctx,
    inspectors::Layered,
    revm::{inspector::NoOpInspector, Database, DatabaseCommit, Inspector},
    TrevmBuilder,
};

pub(crate) const BASE_GAS: usize = 21_000;

/// Type alias for EVMs using a [`StateProviderBox`] as the `DB` type for
/// trevm.
///
/// [`StateProviderBox`]: reth::providers::StateProviderBox
pub type RuRevmState = reth::revm::db::State<
    reth::revm::database::StateProviderDatabase<reth::providers::StateProviderBox>,
>;

/// Create a new EVM with the given database.
pub fn signet_evm<Db: Database + DatabaseCommit>(
    db: Db,
    constants: SignetSystemConstants,
) -> EvmNeedsCfg<Db> {
    TrevmBuilder::new()
        .with_db(db)
        .with_insp(Layered::new(NoOpInspector, OrderDetector::new(constants)))
        .build_trevm()
        .expect("db set")
}

/// Create a new EVM with the given database and inspector.
pub fn signet_evm_with_inspector<Db, I>(
    db: Db,
    inner: I,
    constants: SignetSystemConstants,
) -> EvmNeedsCfg<Db, I>
where
    I: Inspector<Ctx<Db>>,
    Db: Database + DatabaseCommit,
{
    let inspector = SignetLayered::new(inner, OrderDetector::new(constants));

    TrevmBuilder::new().with_db(db).with_insp(inspector).build_trevm().expect("db set")
}

/// Test utilities for the Signet EVM impl.
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils {
    use reth::revm::{context::CfgEnv, primitives::hardfork::SpecId};
    use signet_types::test_utils::*;
    use trevm::revm::database::in_memory_db::InMemoryDB;

    /// Create a new Signet EVM with an in-memory database for testing.
    pub fn test_signet_evm() -> super::EvmNeedsBlock<trevm::revm::database::in_memory_db::InMemoryDB>
    {
        super::signet_evm(InMemoryDB::default(), TEST_CONSTANTS).fill_cfg(&TestCfg)
    }

    /// Test configuration for the Signet EVM.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TestCfg;

    impl trevm::Cfg for TestCfg {
        fn fill_cfg_env(&self, cfg_env: &mut reth::revm::context::CfgEnv) {
            let CfgEnv { chain_id, spec, .. } = cfg_env;

            *chain_id = TEST_RU_CHAIN_ID;
            *spec = SpecId::default();
        }
    }
}
