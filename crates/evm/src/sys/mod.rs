mod driver;

mod logs;
pub use logs::{
    MintNative as MintNativeSysLog, MintToken as MintTokenSysLog, Transact as TransactSysLog,
};

mod native;
pub use native::MintNative;

mod token;
pub use token::MintToken;

mod transact;
pub use transact::TransactSysTx;

use alloy::{
    consensus::ReceiptEnvelope,
    primitives::{Address, Bytes, Log, TxKind},
};
use core::fmt;
use signet_types::primitives::TransactionSigned;
use trevm::{
    helpers::Ctx,
    revm::{context::result::EVMError, Database, DatabaseCommit, Inspector},
    Trevm, Tx,
};

/// [`SysBase`] is the root trait for all system actions and transactions. It
/// provides the basic functionality that the [`SignetDriver`] needs to process
/// system actions and transactions.
pub trait SysBase: fmt::Debug + Clone {
    /// Check if the system action has a nonce. This is typically used to
    /// determine if the nonce should be populated by the Evm during
    /// transaction processing.
    fn has_nonce(&self) -> bool;

    /// Populate the nonce for the transaction. This is typically used to
    /// ensure that the transaction is unique. It will be called by the Evm
    /// during transaction processing to set the nonce for the transaction.
    fn populate_nonce(&mut self, nonce: u64);

    /// Set the nonce for the transaction. This is a convenience method that
    /// calls [`Self::populate_nonce`] with the given nonce.
    fn with_nonce(mut self, nonce: u64) -> Self
    where
        Self: Sized,
    {
        self.populate_nonce(nonce);
        self
    }

    /// Convert the system action into a transaction that can be appended to a
    /// block by the [`SignetDriver`].
    fn produce_transaction(&self) -> TransactionSigned;

    /// Produce a log for the system action. This will be appended to the end
    /// of the receipt, and
    fn produce_log(&self) -> Log;

    /// Get the address that the Signet EVM considers to be the sender of the
    /// system action. This is typically the [`MINTER_ADDRESS`] for token or
    /// native asset mints, and the host-chain user address for transact events.
    ///
    /// [`MINTER_ADDRESS`]: signet_types::constants::MINTER_ADDRESS
    fn evm_sender(&self) -> Address;
}

/// A transaction that is run on the EVM, and may or may not pay gas.
///
/// See [`MeteredSysTx`] and [`UnmeteredSysTx`] for more specific
/// transaction types.
pub trait SysTx: SysBase + Tx {}

/// System actions are operations that apply changes to the EVM state without
/// going through the transaction processing pipeline. They are not run as
/// transactions, and do not have gas limits or revert semantics. They are
/// typically used for operations that need to be applied directly to the state,
/// such as modifying balances.
pub trait SysAction: SysBase {
    /// Apply the system action to the EVM state.
    fn apply<Db, Insp, State>(
        &self,
        evm: &mut Trevm<Db, Insp, State>,
    ) -> Result<(), EVMError<Db::Error>>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>;

    /// Produce a receipt for the system action. This receipt will be
    /// accumulated in the block object during EVM execution.
    fn produce_receipt(&self, cumulative_gas_used: u64) -> ReceiptEnvelope;
}

/// System transactions run on the EVM as a transaction, but do not pay gas and
/// cannot produce Orders. They are run as transactions, but are not subject to
/// the same rules and constraints as regular transactions. They CAN revert,
/// and CAN halt. They are typically used for operations that need to be run as
/// transactions, but should not pay gas. E.g. minting tokens or performing
/// system-level operations that do not require gas payment.
pub trait UnmeteredSysTx: SysBase + Tx {}

/// System transactions run on the EVM as a transaction, and are subject to the
/// same rules and constraints as regular transactions. They may run arbitrary
/// execution, have gas limits, and can revert if they fail. They must satisfy
/// the system market constraints on Orders.
///
/// They are distinct from [`UnmeteredSysTx`], which are run as transactions,
/// but do not pay gas and cannot produce Orders.
///
/// They are distinct from [`SysAction`], which are not run as transactions,
/// but rather apply changes to the state directly without going through the
/// transaction processing pipeline.
pub trait MeteredSysTx: SysBase + Tx {
    /// Get the gas limit for the transaction. This is the maximum amount of
    /// gas that the transaction is allowed to consume.
    ///
    /// Metered system transactions ALWAYS consume all gas.
    fn gas_limit(&self) -> u128;

    /// Get the callee address for the transaction.
    fn callee(&self) -> TxKind;

    /// Get the input data for the transaction. This is the calldata that is
    /// passed to the callee when the transaction is executed.
    fn input(&self) -> Bytes;
}
