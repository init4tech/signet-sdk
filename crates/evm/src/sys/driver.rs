#![allow(dead_code)] // NB: future proofing.

use crate::{
    driver::ControlFlow,
    sys::{
        MeteredSysTx, MintNative, MintToken, SysAction, SysOutput, TransactSysTx, UnmeteredSysTx,
    },
    EvmNeedsTx, RunTxResult, SignetDriver,
};
use alloy::primitives::{map::HashSet, U256};
use signet_extract::Extractable;
use signet_zenith::MINTER_ADDRESS;
use tracing::{debug, debug_span};
use trevm::{
    fillers::{DisableGasChecks, DisableNonceCheck},
    helpers::Ctx,
    revm::{
        context::result::{EVMError, ExecutionResult},
        Database, DatabaseCommit, Inspector,
    },
    trevm_try,
};

/// Populate the nonce for a system output.
fn populate_nonce_from_trevm<Db, Insp, S>(
    trevm: &mut EvmNeedsTx<Db, Insp>,
    sys_output: &mut S,
) -> Result<(), EVMError<Db::Error>>
where
    Db: Database + DatabaseCommit,
    Insp: Inspector<Ctx<Db>>,
    S: SysOutput,
{
    // If the sys_output already has a nonce, we don't need to populate it.
    if sys_output.has_nonce() {
        return Ok(());
    }

    // Read the nonce from the database and populate it in the sys_output.
    trevm
        .try_read_nonce(sys_output.sender())
        .map(|nonce| sys_output.populate_nonce(nonce))
        .map_err(EVMError::Database)
}

impl<'a, 'b, C: Extractable> SignetDriver<'a, 'b, C> {
    /// Apply a [`SysAction`] to the EVM state.
    ///
    /// This will do the following:
    /// - Run the system action, allowing direct EVM state changes.
    /// - Produce the transaction using [`SysOutput::produce_transaction`].
    /// - Produce the syslog eipt using [`SysOutput::produce_log`].
    /// - Produce a receipt containing the gas used and logs.
    /// - Push the resulting transaction to the block.
    /// - Push the resulting receipt to the output.
    ///
    /// [`SysAction`]s have the following properties:
    /// - DO NOT pay for gas.
    /// - DO update the nonce of the [`SysOutput::sender`] sender.
    /// - DO NOT run the EVM.
    ///
    /// See the [`SysAction`] trait documentation for more details.
    ///
    /// [`SysOutput::sender`]: crate::sys::SysOutput::sender
    /// [`SysOutput::produce_log`]: crate::sys::SysOutput::produce_log
    /// [`SysOutput::produce_transaction`]: crate::sys::SysOutput::produce_transaction
    pub(crate) fn apply_sys_action_single<Db, Insp, S>(
        &mut self,
        mut trevm: EvmNeedsTx<Db, Insp>,
        mut action: S,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
        S: SysAction,
    {
        // Populate the nonce for the action.
        trevm_try!(populate_nonce_from_trevm(&mut trevm, &mut action), trevm);

        // Run the system action.
        trevm_try!(action.apply(&mut trevm), trevm);
        // push receipt and transaction to the block
        self.processed.push(action.produce_transaction());
        self.output
            .push_result(action.produce_receipt(self.cumulative_gas_used()), action.sender());

        Ok(trevm)
    }

    /// Apply a series of [`SysAction`]s to the EVM state.
    pub(crate) fn apply_sys_actions<Db, Insp, S>(
        &mut self,
        mut trevm: EvmNeedsTx<Db, Insp>,
        sys_actions: impl IntoIterator<Item = S>,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
        S: SysAction,
    {
        for action in sys_actions {
            trevm = self.apply_sys_action_single(trevm, action)?;
        }
        Ok(trevm)
    }

    /// Inner logic for applying a [`UnmeteredSysTx`] to the EVM state.
    ///
    /// This function expects that gas and nonce checks are already disabled
    /// in the EVM, and will not re-enable them.
    pub(crate) fn apply_unmetered_sys_transaction_inner<Db, Insp, S>(
        &mut self,
        mut trevm: EvmNeedsTx<Db, Insp>,
        mut sys_tx: S,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
        S: UnmeteredSysTx,
    {
        // Populate the nonce for the action.
        trevm_try!(populate_nonce_from_trevm(&mut trevm, &mut sys_tx), trevm);

        // Run the transaction.
        let mut t = run_tx_early_return!(self, trevm, &sys_tx, MINTER_ADDRESS);

        // push a sys_log to the outcome
        if let ExecutionResult::Success { logs, .. } = t.result_mut_unchecked() {
            logs.push(sys_tx.produce_log());
        }

        let tx = sys_tx.produce_transaction();
        Ok(self.accept_tx(t, tx))
    }

    /// Apply a [`UnmeteredSysTx`] to the EVM state.
    ///
    /// When applying many system transactions, it is recommended to use
    /// [`Self::apply_unmetered_sys_transactions`] instead, as it will disable
    /// gas and nonce checks only once, rather than for each transaction.
    ///
    /// This will do the following:
    /// - Disable gas and nonce checks in the EVM.
    /// - Run the system transaction in the EVM as
    ///   [`Self::apply_unmetered_sys_transaction_inner`].
    /// - Re-enable gas and nonce checks in the EVM.
    ///
    /// [`UnmeteredSysTx`]s have the following properties:
    /// - DO NOT pay for gas.
    /// - DO update the nonce of the [`SysOutput::sender`].
    /// - DO run the EVM.
    ///
    /// [`SysOutput::sender`]: crate::sys::SysOutput::sender
    pub(crate) fn apply_unmetered_sys_transaction_single<Db, Insp, S>(
        &mut self,
        trevm: EvmNeedsTx<Db, Insp>,
        sys_tx: S,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
        S: UnmeteredSysTx,
    {
        trevm.try_with_cfg(&DisableGasChecks, |trevm| {
            trevm.try_with_cfg(&DisableNonceCheck, |trevm| {
                self.apply_unmetered_sys_transaction_inner(trevm, sys_tx)
            })
        })
    }

    /// Apply a series of [`UnmeteredSysTx`]s to the EVM state.
    ///
    /// This will do the following:
    /// - Disable gas and nonce checks in the EVM.
    /// - Run each system transaction in the EVM as
    ///   [`Self::apply_unmetered_sys_transaction_inner`].
    /// - Re-enable gas and nonce checks in the EVM.
    ///
    ///     /// [`UnmeteredSysTx`]s have the following properties:
    /// - DO NOT pay for gas.
    /// - DO update the nonce of the [`SysOutput::sender`].
    /// - DO run the EVM.
    ///
    /// [`SysOutput::sender`]: crate::sys::SysOutput::sender
    pub(crate) fn apply_unmetered_sys_transactions<Db, Insp, S>(
        &mut self,
        trevm: EvmNeedsTx<Db, Insp>,
        sys_txs: impl IntoIterator<Item = S>,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
        S: UnmeteredSysTx,
    {
        trevm.try_with_cfg(&DisableGasChecks, |trevm| {
            trevm.try_with_cfg(&DisableNonceCheck, |mut trevm| {
                for sys_tx in sys_txs {
                    // Populate the nonce for the transaction
                    trevm = self.apply_unmetered_sys_transaction_inner(trevm, sys_tx)?;
                }
                Ok(trevm)
            })
        })
    }

    /// Apply a [`MeteredSysTx`] to the EVM state.
    ///
    /// This will do the following:
    /// - Run the system transaction in the EVM.
    /// - Double-check that the sender has enough balance to pay for the unused
    ///   gas.
    /// - Produce the transaction using [`SysOutput::produce_transaction`].
    /// - Produce a syslog using [`SysOutput::produce_log`].
    /// - Push the syslog to the outcome.
    /// - Invoke [`Self::check_fills_and_accept`] to check the fills and
    ///   accept the transaction and receipt.
    ///
    /// [`MeteredSysTx`]s have the following properties:
    /// - DO pay for gas, INCLUDING unused gas.
    /// - DO update the nonce of the [`SysOutput::sender`].
    /// - DO run the EVM.
    ///
    /// [`SysOutput::produce_transaction`]: crate::sys::SysOutput::produce_transaction
    /// [`SysOutput::sender`]: crate::sys::SysOutput::sender
    /// [`SysOutput::produce_log`]: crate::sys::SysOutput::produce_log
    pub(crate) fn apply_metered_sys_transaction_single<Db, Insp, S>(
        &mut self,
        mut trevm: EvmNeedsTx<Db, Insp>,
        mut sys_tx: S,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
        S: MeteredSysTx,
    {
        // Populate the nonce for the action.
        trevm_try!(populate_nonce_from_trevm(&mut trevm, &mut sys_tx), trevm);

        let mut t = run_tx_early_return!(self, trevm, &sys_tx, sys_tx.sender());

        {
            // NB: This is a little sensitive.
            // Although the EVM performs a check on the balance of the sender,
            // to ensure they can pay the full price, that check may be
            // invalidated by transaction execution. As a result, we have to
            // perform the same check here, again.
            let gas_used = t.result().gas_used();

            // Set gas used to the transact gas limit
            match t.result_mut_unchecked() {
                ExecutionResult::Success { ref mut gas_used, .. }
                | ExecutionResult::Revert { ref mut gas_used, .. }
                | ExecutionResult::Halt { ref mut gas_used, .. } => {
                    *gas_used = if sys_tx.gas_limit() >= u64::MAX as u128 {
                        u64::MAX
                    } else {
                        sys_tx.gas_limit() as u64
                    }
                }
            }

            let unused_gas = sys_tx.gas_limit().saturating_sub(gas_used as u128);
            let base_fee = t.block().basefee as u128;
            let to_debit = base_fee * unused_gas;

            debug!(%base_fee, gas_used, %unused_gas, %to_debit, "Debiting unused transact gas");

            let acct = t
                .result_and_state_mut_unchecked()
                .state
                .get_mut(&sys_tx.sender())
                .expect("sender account must be in state, as it is touched by definition");

            match acct.info.balance.checked_sub(U256::from(to_debit)) {
                // If the balance is sufficient, debit the account.
                Some(balance) => acct.info.balance = balance,
                // If the balance is insufficient, discard the transaction.
                None => {
                    debug!("Discarding metered sys tx outcome due to insufficient balance to pay for unused gas");
                    return Ok(t.reject());
                }
            }
        }

        // push a sys_log to the outcome
        if let ExecutionResult::Success { logs, .. } = t.result_mut_unchecked() {
            logs.push(sys_tx.produce_log());
        }

        let tx = sys_tx.produce_transaction();
        self.check_fills_and_accept(t, tx)
    }

    /// Apply a series of [`MeteredSysTx`]s to the EVM state.
    ///
    /// This will do the following:
    /// - Run each system transaction in the EVM using
    ///   [`Self::apply_metered_sys_transaction_single`].
    ///
    /// [`MeteredSysTx`]s have the following properties:
    /// - DO pay for gas, INCLUDING unused gas.
    /// - DO update the nonce of the [`SysOutput::sender`].
    /// - DO run the EVM.
    ///
    /// [`SysOutput::sender`]: crate::sys::SysOutput::sender
    fn apply_metered_sys_transactions<Db, Insp, S>(
        &mut self,
        mut trevm: EvmNeedsTx<Db, Insp>,
        sys_txs: impl IntoIterator<Item = S>,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
        S: MeteredSysTx,
    {
        for mut sys_tx in sys_txs {
            let span = tracing::debug_span!(
                "SignetDriver::apply_metered_sys_transactions",
                sender = %sys_tx.sender(),
                gas_limit = sys_tx.gas_limit(),
                callee = ?sys_tx.callee(),
            );
            if tracing::enabled!(tracing::Level::TRACE) {
                span.record("input", format!("{}", &sys_tx.input()));
            }
            let _enter = span.entered();

            let nonce = trevm_try!(
                trevm.try_read_nonce(sys_tx.sender()).map_err(EVMError::Database),
                trevm
            );
            sys_tx.populate_nonce(nonce);
            debug!(nonce, "Applying metered sys tx");
            trevm = self.apply_metered_sys_transaction_single(trevm, sys_tx)?;
        }
        Ok(trevm)
    }

    /// Execute all [`Transactor::Transact`] extracts from the block via
    /// [`Self::apply_metered_sys_transactions`].
    pub(crate) fn execute_all_transacts<Db, Insp>(
        &mut self,
        trevm: EvmNeedsTx<Db, Insp>,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
    {
        let _span = tracing::debug_span!(
            "SignetDriver::execute_all_transacts",
            count = self.extracts.transacts.len()
        )
        .entered();
        let transacts = self.extracts.transacts.iter().map(TransactSysTx::new);
        self.apply_metered_sys_transactions(trevm, transacts)
    }

    /// Run all mints, including enters and enter tokens.
    ///
    /// This could be implemented more-simply using
    /// [`Self::apply_unmetered_sys_transactions`] and
    /// [`Self::apply_sys_actions`],
    ///
    /// This is special cased as follows:
    /// - Nonce lookups are done ONCE as the sender is known to be identical
    ///   for all mints.
    /// - Details are collected for tracing purposes.
    /// - Nonce setting is done at the end, after all mints are processed.
    fn run_mints_inner<Db, Insp>(
        &mut self,
        mut trevm: EvmNeedsTx<Db, Insp>,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
    {
        // Load the nonce once, we'll write it at the end
        let minter_nonce =
            trevm_try!(trevm.try_read_nonce(MINTER_ADDRESS).map_err(EVMError::Database), trevm);

        // Some setup for logging
        let _span = debug_span!(
            "signet_evm::evm::run_mints",
            enters = self.extracts.enters.len(),
            enter_tokens = self.extracts.enter_tokens.len(),
            minter_nonce
        );
        let mut eth_minted = U256::ZERO;
        let mut eth_accts = HashSet::with_capacity(self.extracts.enters.len());
        let mut usd_minted = U256::ZERO;
        let mut usd_accts = HashSet::with_capacity(self.extracts.enter_tokens.len());

        let eth_token = self.constants.rollup().tokens().weth();

        for (i, e) in self.extracts.enters.iter().enumerate() {
            let mut mint = MintToken::from_enter(eth_token, e);
            mint.populate_nonce(minter_nonce + i as u64);
            trevm = self.apply_unmetered_sys_transaction_inner(trevm, mint)?;

            eth_minted += e.event.amount;
            eth_accts.insert(e.event.recipient());
        }

        // Use a new base nonce for the enter_tokens
        let minter_nonce = minter_nonce + self.extracts.enters.len() as u64;

        for (i, e) in self.extracts.enter_tokens.iter().enumerate() {
            let nonce = minter_nonce + i as u64;
            if self.constants.is_host_usd(e.event.token) {
                // USDC is handled as a native mint
                let mut mint = MintNative::new(e);
                mint.populate_nonce(nonce);
                trevm = self.apply_sys_action_single(trevm, mint)?;
                usd_minted += e.event.amount;
                usd_accts.insert(e.event.recipient());
            } else {
                // All other tokens are non-native mints
                let ru_token_addr = self
                    .constants
                    .rollup_token_from_host_address(e.event.token)
                    .expect("token enters must be permissioned");
                let mut mint = MintToken::from_enter_token(ru_token_addr, e);
                mint.populate_nonce(nonce);
                trevm = self.apply_unmetered_sys_transaction_inner(trevm, mint)?;
            }
        }

        // Update the minter nonce.
        let minter_nonce = minter_nonce + self.extracts.enter_tokens.len() as u64;
        trevm_try!(
            trevm.try_set_nonce_unchecked(MINTER_ADDRESS, minter_nonce).map_err(EVMError::Database),
            trevm
        );

        debug!(
            %eth_minted,
            eth_accts_touched = %eth_accts.len(),
            %usd_minted,
            usd_accts_touched = %usd_accts.len(),
            "Minting completed"
        );

        Ok(trevm)
    }

    pub(crate) fn run_all_mints<Db, Insp>(
        &mut self,
        trevm: EvmNeedsTx<Db, Insp>,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
    {
        trevm.try_with_cfg(&DisableGasChecks, |trevm| {
            trevm.try_with_cfg(&DisableNonceCheck, |trevm| self.run_mints_inner(trevm))
        })
    }
}
