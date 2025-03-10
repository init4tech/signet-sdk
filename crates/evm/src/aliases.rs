use super::*;
#[allow(unused_imports)]
use trevm::{
    revm::{inspectors::NoOpInspector, primitives::EVMError, Database},
    Block, Cfg, Trevm,
};

/// A [`Trevm`] that requires a [`Cfg`].
///
/// Expected continuations include:
/// - [`EvmNeedsCfg::fill_cfg`]
///
/// [`Cfg`]: trevm::Cfg
/// [`Trevm`]: trevm::Trevm
pub type EvmNeedsCfg<'a, Db, I = NoOpInspector> = trevm::EvmNeedsCfg<'a, OrderDetector<I>, Db>;

/// A [`Trevm`] that requires a [`Block`] and contains no
/// outputs. This EVM has not yet executed any transactions or state changes.
///
/// Expected continuations include:
/// - [`EvmNeedsBlock::fill_block`]
///
/// [`Block`]: trevm::Block
/// [`Trevm`]: trevm::Trevm
pub type EvmNeedsBlock<'a, Db, I = NoOpInspector> = trevm::EvmNeedsBlock<'a, OrderDetector<I>, Db>;

/// A [`Trevm`] that requires a [`Tx`].
///
/// Expected continuations include:
/// - [`EvmNeedsTx::fill_tx`]
/// - [`EvmNeedsTx::run_tx`]
/// - [`EvmNeedsTx::finish`]
///
/// [`Tx`]: trevm::Tx
/// [`Trevm`]: trevm::Trevm
pub type EvmNeedsTx<'a, Db, I = NoOpInspector> = trevm::EvmNeedsTx<'a, OrderDetector<I>, Db>;

/// A [`Trevm`] that is ready to execute a transaction.
///
/// The transaction may be executed with [`EvmReady::run`] or cleared
/// with [`EvmReady::clear_tx`].
///
/// [`Trevm`]: trevm::Trevm
pub type EvmReady<'a, Db, I = NoOpInspector> = trevm::EvmReady<'a, OrderDetector<I>, Db>;

/// A [`Trevm`] that encountered an error during transaction execution.
///
/// Expected continuations include:
/// - [`EvmTransacted::reject`]
/// - [`EvmTransacted::accept`]
///
/// [`Trevm`]: trevm::Trevm
pub type EvmTransacted<'a, Db, I = NoOpInspector> = trevm::EvmTransacted<'a, OrderDetector<I>, Db>;

/// A [`Trevm`] that encountered an error during transaction execution.
///
/// Expected continuations include:
/// - [`EvmErrored::discard_error`]
/// - [`EvmErrored::into_error`]
///
/// [`Trevm`]: trevm::Trevm
pub type EvmErrored<'a, Db, I = NoOpInspector, E = EVMError<<Db as Database>::Error>> =
    trevm::EvmErrored<'a, OrderDetector<I>, Db, E>;

/// The result of running transactions for a block driver.
pub type RunTxResult<'a, Db, T, I = NoOpInspector> =
    trevm::RunTxResult<'a, OrderDetector<I>, Db, T>;
