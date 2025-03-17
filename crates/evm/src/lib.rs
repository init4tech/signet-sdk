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

mod orders;
pub use orders::{Framed, FramedFilleds, FramedOrders, OrderDetector};

mod result;
pub use result::BlockResult;

use signet_types::config::SignetSystemConstants;
use trevm::{
    revm::{
        inspector_handle_register, inspectors::NoOpInspector, Database, DatabaseCommit, EvmBuilder,
        Inspector,
    },
    TrevmBuilder,
};

pub(crate) const BASE_GAS: usize = 21_000;

/// Type alias for EVMs using a [`StateProviderBox`] as the `DB` type for
/// trevm.
pub type RuRevmState = reth::revm::db::State<
    reth::revm::database::StateProviderDatabase<reth::providers::StateProviderBox>,
>;

/// Create a new EVM with the given database.
pub fn signet_evm<Db: Database + DatabaseCommit>(
    db: Db,
    constants: SignetSystemConstants,
) -> EvmNeedsCfg<'static, Db> {
    EvmBuilder::default()
        .with_db(db)
        .with_external_context(OrderDetector::<NoOpInspector>::new(constants))
        .append_handler_register(inspector_handle_register)
        .build_trevm()
}

/// Create a new EVM with the given database and inspector.
pub fn signet_evm_with_inspector<Db, I>(
    db: Db,
    inner: I,
    constants: SignetSystemConstants,
) -> EvmNeedsCfg<'static, Db, I>
where
    I: Inspector<Db>,
    Db: Database + DatabaseCommit,
{
    let inspector = OrderDetector::new_with_inspector(constants, inner);
    EvmBuilder::default()
        .with_db(db)
        .with_external_context(inspector)
        .append_handler_register(inspector_handle_register)
        .build_trevm()
}

/// Test utilities for the Signet EVM impl.
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils {

    use reth::revm::InMemoryDB;
    use signet_types::test_utils::*;

    /// Create a new Signet EVM with an in-memory database for testing.
    pub fn test_signet_evm() -> super::EvmNeedsCfg<'static, trevm::revm::db::InMemoryDB> {
        let mut trevm = super::signet_evm(InMemoryDB::default(), TEST_CONSTANTS);
        trevm.inner_mut_unchecked().cfg_mut().chain_id = TEST_RU_CHAIN_ID;
        trevm
    }
}
