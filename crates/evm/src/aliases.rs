use super::*;
#[allow(unused_imports)]
use trevm::{
    revm::{context::result::EVMError, inspector::NoOpInspector, Database},
    Block, Cfg, Trevm,
};

/// A [`Trevm`] that requires a [`Cfg`].
///
/// Expected continuations include:
/// - [`EvmNeedsCfg::fill_cfg`]
///
/// [`Cfg`]: trevm::Cfg
/// [`Trevm`]: trevm::Trevm
pub type EvmNeedsCfg<Db, I = NoOpInspector> = trevm::EvmNeedsCfg<Db, OrderDetector<I>>;

/// A [`Trevm`] that requires a [`Block`] and contains no
/// outputs. This EVM has not yet executed any transactions or state changes.
///
/// Expected continuations include:
/// - [`EvmNeedsBlock::fill_block`]
///
/// [`Block`]: trevm::Block
/// [`Trevm`]: trevm::Trevm
pub type EvmNeedsBlock<Db, I = NoOpInspector> = trevm::EvmNeedsBlock<Db, OrderDetector<I>>;

/// A [`Trevm`] that requires a [`Tx`].
///
/// Expected continuations include:
/// - [`EvmNeedsTx::fill_tx`]
/// - [`EvmNeedsTx::run_tx`]
/// - [`EvmNeedsTx::finish`]
///
/// [`Tx`]: trevm::Tx
/// [`Trevm`]: trevm::Trevm
pub type EvmNeedsTx<Db, I = NoOpInspector> = trevm::EvmNeedsTx<Db, OrderDetector<I>>;

/// A [`Trevm`] that is ready to execute a transaction.
///
/// The transaction may be executed with [`EvmReady::run`] or cleared
/// with [`EvmReady::clear_tx`].
///
/// [`Trevm`]: trevm::Trevm
pub type EvmReady<Db, I = NoOpInspector> = trevm::EvmReady<Db, OrderDetector<I>>;

/// A [`Trevm`] that encountered an error during transaction execution.
///
/// Expected continuations include:
/// - [`EvmTransacted::reject`]
/// - [`EvmTransacted::accept`]
///
/// [`Trevm`]: trevm::Trevm
pub type EvmTransacted<Db, I = NoOpInspector> = trevm::EvmTransacted<Db, OrderDetector<I>>;

/// A [`Trevm`] that encountered an error during transaction execution.
///
/// Expected continuations include:
/// - [`EvmErrored::discard_error`]
/// - [`EvmErrored::into_error`]
///
/// [`Trevm`]: trevm::Trevm
pub type EvmErrored<Db, I = NoOpInspector, E = EVMError<<Db as Database>::Error>> =
    trevm::EvmErrored<Db, OrderDetector<I>, E>;

/// The result of running transactions for a block driver.
pub type RunTxResult<Db, T, I = NoOpInspector> = trevm::RunTxResult<Db, OrderDetector<I>, T>;

/// The result of driving a bundle.
pub type DriveBundleResult<Db, T, I = NoOpInspector> =
    trevm::DriveBundleResult<Db, OrderDetector<I>, T>;
