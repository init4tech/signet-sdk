mod logs;
pub use logs::{
    MintNative as MintNativeSysLog, MintToken as MintTokenSysLog, Transact as TransactSysLog,
};

mod native;
pub use native::MintNative;

mod token;
pub use token::MintToken;

mod transact;

use alloy::{
    consensus::ReceiptEnvelope,
    primitives::{Address, Log},
};
use core::fmt;
use signet_types::primitives::TransactionSigned;
use trevm::{
    helpers::Ctx,
    revm::{context::result::EVMError, Database, DatabaseCommit, Inspector},
    Trevm, Tx,
};

/// Produce a transaction from a system action. This will be ingested into the
/// block during EVM execution.
pub trait SysOutput: fmt::Debug + Clone {
    /// Convert the system action into a transaction that can be appended to a
    /// block by the [`SignetDriver`].
    fn produce_transaction(&self) -> TransactionSigned;

    /// Produce a log for the system action. This will be appended to the end
    /// of the receipt, and
    fn produce_log(&self) -> Log;

    /// Get the address of the sender of the system action. This is typically
    /// the [`MINTER_ADDRESS`] for minting actions, or the address of the
    /// system contract caller for other actions.
    fn sender(&self) -> Address;
}

/// System transactions run on the EVM as a transaction, and are subject to the
/// same rules and constraints as regular transactions. They may run arbitrary
/// execution, have gas limits, and can revert if they fail. They must satisfy
/// the system market constraints on Orders.
///
/// They are distinct from [`SysAction`], which are not run as transactions,
/// but rather apply changes to the state directly without going through the
/// transaction processing pipeline.
pub trait SysTx: SysOutput + Tx {}

impl<T> SysTx for T where T: SysOutput + Tx {}

/// System actions are operations that apply changes to the EVM state without
/// going through the transaction processing pipeline. They are not run as
/// transactions, and do not have gas limits or revert semantics. They are
/// typically used for operations that need to be applied directly to the state,
/// such as modifying balances.
pub trait SysAction: SysOutput {
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
