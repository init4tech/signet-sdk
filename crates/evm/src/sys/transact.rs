use crate::{ControlFlow, EvmNeedsTx, RunTxResult, SignetDriver};
use alloy::primitives::U256;
use signet_extract::{Extractable, ExtractedEvent};
use signet_zenith::Transactor;
use tracing::{debug, debug_span};
use trevm::{
    fillers::DisableNonceCheck,
    helpers::Ctx,
    revm::{
        context::{
            result::{EVMError, ExecutionResult},
            TxEnv,
        },
        Database, DatabaseCommit, Inspector,
    },
    trevm_try, Tx,
};

/// Shim to impl [`Tx`] for [`Transactor::Transact`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TransactFiller<'a, 'b, R> {
    /// The extracted event for the transact event.
    pub transact: &'a ExtractedEvent<'b, R, Transactor::Transact>,
    /// The nonce of the transaction.
    pub nonce: u64,
}

impl<R: Sync> Tx for TransactFiller<'_, '_, R> {
    fn fill_tx_env(&self, tx_env: &mut TxEnv) {
        self.transact.event.fill_tx_env(tx_env);
        tx_env.nonce = self.nonce;
    }
}

impl<'a, 'b, C> SignetDriver<'a, 'b, C>
where
    C: Extractable,
{
    /// Execute a [`Transactor::Transact`] event.
    ///
    /// This function does the following:
    /// - Run the transaction.
    /// - Check the aggregate fills.
    /// - Debit the sender's account for unused gas.
    /// - Create a receipt.
    /// - Create a transaction and push it to the block.
    ///
    /// [`Transactor::Transact`]: signet_zenith::Transactor::Transact
    fn execute_transact_event<Db, Insp>(
        &mut self,
        mut trevm: EvmNeedsTx<Db, Insp>,
        idx: usize,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
    {
        let _span = {
            let e = &self.extracts.transacts[idx];
            debug_span!("execute_transact_event", idx,
                host_tx = %e.tx_hash(),
                log_index = e.log_index,
                sender = %e.event.sender,
                gas_limit = e.event.gas(),
            )
            .entered()
        };

        let sender = self.extracts.transacts[idx].event.sender;
        let nonce = trevm_try!(trevm.try_read_nonce(sender).map_err(EVMError::Database), trevm);

        let transact = &self.extracts.transacts[idx];
        let to_execute = TransactFiller { transact, nonce };

        let mut t = run_tx_early_return!(self, trevm, &to_execute, sender);

        {
            // NB: This is a little sensitive.
            // Although the EVM performs a check on the balance of the sender,
            // to ensure they can pay the full price, that check may be
            // invalidated by transaction execution. As a result, we have to
            // perform the same check here, again.
            let gas_used = t.result().gas_used();

            // Set gas used to the transact gas limit
            match t.result_mut_unchecked() {
                ExecutionResult::Success { gas_used, .. }
                | ExecutionResult::Revert { gas_used, .. }
                | ExecutionResult::Halt { gas_used, .. } => {
                    *gas_used = if transact.gas() >= u64::MAX as u128 {
                        u64::MAX
                    } else {
                        transact.gas() as u64
                    }
                }
            }

            let unused_gas = transact.gas.saturating_sub(U256::from(gas_used));
            let base_fee = t.block().basefee;
            let to_debit = U256::from(base_fee) * unused_gas;

            debug!(%base_fee, gas_used, %unused_gas, %to_debit, "Debiting unused transact gas");

            let acct = t
                .result_and_state_mut_unchecked()
                .state
                .get_mut(&transact.sender)
                .expect("sender account must be in state, as it is touched by definition");

            match acct.info.balance.checked_sub(to_debit) {
                // If the balance is sufficient, debit the account.
                Some(balance) => acct.info.balance = balance,
                // If the balance is insufficient, discard the transaction.
                None => {
                    debug!("Discarding transact outcome due to insufficient balance to pay for unused transact gas");
                    return Ok(t.reject());
                }
            }
        }

        // Convert the transact event into a transaction, and then check the
        // aggregate fills
        let tx = transact.make_transaction(nonce);
        self.check_fills_and_accept(t, tx, Some(transact))
    }

    /// Execute all transact events.
    pub(crate) fn execute_all_transacts<Db, Insp>(
        &mut self,
        trevm: EvmNeedsTx<Db, Insp>,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
    {
        trevm.try_with_cfg(&DisableNonceCheck, |mut trevm| {
            for i in 0..self.extracts.transacts.len() {
                trevm = self.execute_transact_event(trevm, i)?;
            }
            Ok(trevm)
        })
    }
}
