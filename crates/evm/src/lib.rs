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

#[macro_use]
mod macros;

mod aliases;
pub use aliases::*;

mod driver;
pub(crate) use driver::ControlFlow;
pub use driver::SignetDriver;

mod journal;
pub use journal::HostJournal;

mod orders;
pub use orders::{Framed, FramedFilleds, FramedOrders, OrderDetector, SignetInspector};

mod outcome;
pub use outcome::ExecutionOutcome;

mod precompiles;
pub use precompiles::signet_precompiles;

mod result;
pub use result::BlockResult;

use signet_types::constants::SignetSystemConstants;
use trevm::{
    helpers::Ctx,
    inspectors::Layered,
    revm::{inspector::NoOpInspector, Database, DatabaseCommit, Inspector},
    TrevmBuilder,
};

/// System structs and types.
pub mod sys;

/// Create a new EVM with the given database.
pub fn signet_evm<Db: Database + DatabaseCommit>(
    db: Db,
    constants: SignetSystemConstants,
) -> EvmNeedsCfg<Db> {
    TrevmBuilder::new()
        .with_db(db)
        .with_insp(Layered::new(NoOpInspector, OrderDetector::new(constants)))
        .with_precompiles(signet_precompiles())
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

    TrevmBuilder::new()
        .with_db(db)
        .with_insp(inspector)
        .with_precompiles(signet_precompiles())
        .build_trevm()
        .expect("db set")
}
