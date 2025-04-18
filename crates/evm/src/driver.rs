use crate::{
    convert::{Enter, EnterToken, Transact},
    orders::SignetInspector,
    BlockResult, EvmNeedsTx, EvmTransacted, RunTxResult, SignetLayered, ToRethPrimitive, BASE_GAS,
};
use alloy::{
    consensus::{ReceiptEnvelope, Transaction as _},
    eips::eip1559::{BaseFeeParams, INITIAL_BASE_FEE as EIP1559_INITIAL_BASE_FEE},
    primitives::{Address, Bloom, U256},
};
use reth::{
    core::primitives::SignedTransaction,
    primitives::{
        Block, BlockBody, Header, Receipt, RecoveredBlock, SealedHeader, Transaction,
        TransactionSigned, TxType,
    },
    providers::ExecutionOutcome,
    revm::{
        context::{ContextTr, TransactTo},
        context_interface::block::BlobExcessGasAndPrice,
        Inspector,
    },
};
use signet_extract::Extracts;
use signet_types::{config::SignetSystemConstants, AggregateFills, MarketError};
use signet_zenith::MINTER_ADDRESS;
use std::collections::{HashSet, VecDeque};
use tracing::{debug, debug_span, trace_span, warn};
use trevm::{
    fillers::{DisableGasChecks, DisableNonceCheck},
    helpers::Ctx,
    revm::{
        context::{
            result::{EVMError, ExecutionResult},
            BlockEnv, CfgEnv, TxEnv,
        },
        database::State,
        Database, DatabaseCommit,
    },
    trevm_try, BlockDriver, BlockOutput, Tx,
};

macro_rules! run_tx {
    ($self:ident, $trevm:ident, $tx:expr, $sender:expr) => {{
        let trevm = $trevm.fill_tx($tx);

        let _guard = trace_span!("run_tx", block_env = ?trevm.block(), tx = ?$tx, tx_env = ?trevm.tx(), spec_id = ?trevm.spec_id()).entered();

        match trevm.run() {
            Ok(t) => {
                debug!("evm executed successfully");
                ControlFlow::Keep(t)
            },
            Err(e) => {
                if e.is_transaction_error() {
                    debug!(
                        err = %e.as_transaction_error().unwrap(),
                        "Discarding outcome due to execution error"
                    );
                    ControlFlow::Discard(e.discard_error())
                } else {
                    return Err(e.err_into());
                }
            }
        }
    }};
}

macro_rules! run_tx_early_return {
    ($self:ident, $trevm:ident, $tx:expr, $sender:expr) => {
        match run_tx!($self, $trevm, $tx, $sender) {
            ControlFlow::Discard(t) => return Ok(t),
            ControlFlow::Keep(t) => t,
        }
    };
}

/// Used internally to signal that the transaction should be discarded.
enum ControlFlow<Db, Insp>
where
    Db: Database + DatabaseCommit,
    Insp: Inspector<Ctx<Db>>,
{
    Discard(EvmNeedsTx<Db, Insp>),
    Keep(EvmTransacted<Db, Insp>),
}

#[derive(thiserror::Error)]
pub enum SignetDriverError<Db>
where
    Db: Database,
{
    /// A market error occurred.
    #[error("Market error: {0}")]
    MarketError(#[from] MarketError),
    /// An EVM error occurred.
    #[error("EVM error")]
    EVMError(EVMError<Db::Error>),
}

impl<Db: Database> std::fmt::Debug for SignetDriverError<Db> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MarketError(arg0) => f.debug_tuple("MarketError").field(arg0).finish(),
            Self::EVMError(_) => f.debug_tuple("EVMError").finish(),
        }
    }
}

impl<Db> From<EVMError<Db::Error>> for SignetDriverError<Db>
where
    Db: Database,
{
    fn from(e: EVMError<Db::Error>) -> Self {
        Self::EVMError(e)
    }
}

/// Shim to impl [`Tx`] for [`TransactionSigned`]
#[derive(Debug)]
struct FillShim<'a>(&'a TransactionSigned, Address);

impl Tx for FillShim<'_> {
    fn fill_tx_env(&self, tx_env: &mut TxEnv) {
        let TxEnv {
            tx_type,
            caller,
            gas_limit,
            gas_price,
            kind,
            value,
            data,
            nonce,
            chain_id,
            access_list,
            gas_priority_fee,
            blob_hashes,
            max_fee_per_blob_gas,
            authorization_list,
        } = tx_env;

        *caller = self.1;

        match self.0.as_ref() {
            Transaction::Legacy(tx) => {
                *tx_type = TxType::Legacy as u8;
                *gas_limit = tx.gas_limit;
                *gas_price = tx.gas_price;
                *gas_priority_fee = None;
                *kind = tx.to;
                *value = tx.value;
                *data = tx.input.clone();
                *chain_id = tx.chain_id;
                *nonce = tx.nonce;
                access_list.0.clear();
                blob_hashes.clear();
                *max_fee_per_blob_gas = 0;
                authorization_list.clear();
            }
            Transaction::Eip2930(tx) => {
                *tx_type = TxType::Eip2930 as u8;
                *gas_limit = tx.gas_limit;
                *gas_price = tx.gas_price;
                *gas_priority_fee = None;
                *kind = tx.to;
                *value = tx.value;
                *data = tx.input.clone();
                *chain_id = Some(tx.chain_id);
                *nonce = tx.nonce;
                access_list.clone_from(&tx.access_list);
                blob_hashes.clear();
                *max_fee_per_blob_gas = 0;
                authorization_list.clear();
            }
            Transaction::Eip1559(tx) => {
                *tx_type = TxType::Eip1559 as u8;
                *gas_limit = tx.gas_limit;
                *gas_price = tx.max_fee_per_gas;
                *gas_priority_fee = Some(tx.max_priority_fee_per_gas);
                *kind = tx.to;
                *value = tx.value;
                *data = tx.input.clone();
                *chain_id = Some(tx.chain_id);
                *nonce = tx.nonce;
                access_list.clone_from(&tx.access_list);
                blob_hashes.clear();
                *max_fee_per_blob_gas = 0;
                authorization_list.clear();
            }
            Transaction::Eip4844(tx) => {
                *tx_type = TxType::Eip4844 as u8;
                *gas_limit = tx.gas_limit;
                *gas_price = tx.max_fee_per_gas;
                *gas_priority_fee = Some(tx.max_priority_fee_per_gas);
                *kind = TransactTo::Call(tx.to);
                *value = tx.value;
                *data = tx.input.clone();
                *chain_id = Some(tx.chain_id);
                *nonce = tx.nonce;
                access_list.clone_from(&tx.access_list);
                blob_hashes.clone_from(&tx.blob_versioned_hashes);
                *max_fee_per_blob_gas = tx.max_fee_per_blob_gas;
                authorization_list.clear();
            }
            Transaction::Eip7702(tx) => {
                *tx_type = TxType::Eip7702 as u8;
                *gas_limit = tx.gas_limit;
                *gas_price = tx.max_fee_per_gas;
                *gas_priority_fee = Some(tx.max_priority_fee_per_gas);
                *kind = tx.to.into();
                *value = tx.value;
                *data = tx.input.clone();
                *chain_id = Some(tx.chain_id);
                *nonce = tx.nonce;
                access_list.clone_from(&tx.access_list);
                blob_hashes.clear();
                *max_fee_per_blob_gas = 0;
                authorization_list.clone_from(&tx.authorization_list);
            }
        }
    }
}

/// A driver for the Signet EVM
#[derive(Debug)]
pub struct SignetDriver<'a, 'b> {
    /// The block extracts.
    extracts: &'a Extracts<'b>,

    /// Parent rollup block.
    parent: SealedHeader,

    /// Rollup constants, including pre-deploys
    constants: SignetSystemConstants,

    /// The working context is a clone of the block's [`AggregateFills`] that
    /// is updated progessively as the block is evaluated.
    working_context: AggregateFills,

    /// Transactions in the RU block (if any)
    to_process: VecDeque<TransactionSigned>,

    /// Transactions that have been processed.
    processed: Vec<TransactionSigned>,

    /// Receipts and senders.
    output: BlockOutput,

    /// Payable gas used in the block.
    payable_gas_used: u64,
}

impl<'a, 'b> SignetDriver<'a, 'b> {
    /// Create a new driver.
    pub fn new(
        extracts: &'a Extracts<'b>,
        to_process: VecDeque<TransactionSigned>,
        parent: SealedHeader,
        constants: SignetSystemConstants,
    ) -> Self {
        let cap = to_process.len()
            + extracts.transacts.len()
            + extracts.enters.len()
            + extracts.enter_tokens.len();
        Self {
            extracts,
            parent,
            constants,
            working_context: extracts.aggregate_fills(),
            to_process,
            processed: Vec::with_capacity(cap),
            output: BlockOutput::with_capacity(cap),
            payable_gas_used: 0,
        }
    }

    /// Get the extracts being executed by the driver.
    pub const fn extracts(&self) -> &Extracts<'b> {
        self.extracts
    }

    /// Get the parent header.
    pub const fn parent(&self) -> &SealedHeader {
        &self.parent
    }

    /// Get the system constants.
    pub const fn constants(&self) -> &SignetSystemConstants {
        &self.constants
    }

    /// Get the rollup height of the current block.
    pub const fn ru_height(&self) -> u64 {
        self.extracts.ru_height
    }

    /// beneficiary of the current block.
    pub fn beneficiary(&self) -> Address {
        self.extracts.ru_header().map(|h| h.rewardAddress).unwrap_or(self.parent.beneficiary)
    }

    /// Base fee beneficiary of the current block.
    pub const fn base_fee_recipient(&self) -> Address {
        self.constants.base_fee_recipient()
    }

    /// Gas limit of the current block.
    pub fn gas_limit(&self) -> u64 {
        self.extracts.ru_header().map(|h| h.gas_limit()).unwrap_or(self.parent.gas_limit)
    }

    /// Base fee of the current block.
    pub fn base_fee(&self) -> u64 {
        if self.ru_height() == 0 {
            // case should never occur.
            EIP1559_INITIAL_BASE_FEE
        } else {
            self.parent
                .next_block_base_fee(BaseFeeParams::ethereum())
                .unwrap_or(EIP1559_INITIAL_BASE_FEE)
        }
    }

    /// Get the cumulative gas used in the block (so far). This excludes gas
    /// used by enters and enter token events.
    pub const fn payable_gas_used(&self) -> u64 {
        self.payable_gas_used
    }

    /// Get the cumulative gas used in the block, including the gas used by
    /// enters and enter token events.
    pub fn cumulative_gas_used(&self) -> u64 {
        self.output.cumulative_gas_used()
    }

    /// Consume the driver, producing the sealed block and receipts.
    pub fn finish(self) -> (RecoveredBlock<Block>, Vec<Receipt>) {
        let (header, hash) = self.construct_sealed_header().split();
        let (receipts, senders, _) = self.output.into_parts();

        let block = RecoveredBlock::new(
            Block::new(header, BlockBody { transactions: self.processed, ..Default::default() }),
            senders,
            hash,
        );

        let receipts = receipts.into_iter().map(|re| re.to_reth()).collect();

        (block, receipts)
    }

    /// Consume the driver and trevm, producing a [`BlockResult`].
    pub fn finish_trevm<Db, Insp>(self, trevm: crate::EvmNeedsBlock<State<Db>, Insp>) -> BlockResult
    where
        Db: Database,
        Insp: Inspector<Ctx<State<Db>>>,
    {
        let ru_height = self.extracts.ru_height;
        let (sealed_block, receipts) = self.finish();
        BlockResult {
            sealed_block,
            execution_outcome: ExecutionOutcome::new(
                trevm.finish(),
                vec![receipts],
                ru_height,
                vec![],
            ),
        }
    }

    /// Get the logs bloom of the block.
    fn logs_bloom(&self) -> Bloom {
        self.output.logs_bloom()
    }

    /// Make a receipt from the execution result.
    fn make_receipt(&self, result: ExecutionResult) -> alloy::consensus::Receipt {
        let cumulative_gas_used = self.cumulative_gas_used().saturating_add(result.gas_used());
        alloy::consensus::Receipt {
            status: result.is_success().into(),
            cumulative_gas_used,
            logs: result.into_logs(),
        }
    }

    /// Make a receipt for an enter.
    fn make_enter_receipt(&self) -> alloy::consensus::Receipt {
        let cumulative_gas_used = self.cumulative_gas_used().saturating_add(BASE_GAS as u64);
        alloy::consensus::Receipt { status: true.into(), cumulative_gas_used, logs: vec![] }
    }

    /// Construct a block header for DB and evm execution.
    fn construct_header(&self) -> Header {
        Header {
            parent_hash: self.parent.hash(),
            number: self.ru_height(),
            gas_limit: self.gas_limit(),
            timestamp: self.extracts.host_block.timestamp,
            base_fee_per_gas: Some(self.base_fee()),
            beneficiary: self.beneficiary(),

            logs_bloom: self.logs_bloom(),
            gas_used: self.cumulative_gas_used(),

            difficulty: self.extracts.host_block.difficulty,

            mix_hash: self.extracts.host_block.mix_hash,
            nonce: self.extracts.host_block.nonce,
            parent_beacon_block_root: self.extracts.host_block.parent_beacon_block_root,

            ..Default::default()
        }
    }

    /// Construct a sealed header for DB and evm execution.
    fn construct_sealed_header(&self) -> SealedHeader {
        let header = self.construct_header();
        let hash = header.hash_slow();
        SealedHeader::new(header, hash)
    }

    /// Check the [`AggregateFills`], discard if invalid, otherwise accumulate
    /// payable gas and call [`Self::accept_tx`].
    ///
    /// This path is used by
    /// - [`TransactionSigned`] objects
    /// - [`Transact`] events
    fn check_fills_and_accept<Db, Insp>(
        &mut self,
        mut trevm: EvmTransacted<Db, Insp>,
        tx: TransactionSigned,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
    {
        // Taking these clears the context for reuse.
        let (agg_orders, agg_fills) =
            trevm.inner_mut_unchecked().data.inspector.as_mut_detector().take_aggregates();

        // We check the AggregateFills here, and if it fails, we discard the
        // transaction outcome and push a failure receipt.
        if let Err(err) = self.working_context.checked_remove_ru_tx_events(&agg_orders, &agg_fills)
        {
            debug!(%err, "Discarding transaction outcome due to market error");
            return Ok(trevm.reject());
        }

        // We track this separately from the cumulative gas used. Enters and
        // EnterTokens are not payable, so we don't want to include their gas
        // usage in the payable gas used.
        self.payable_gas_used += trevm.result().gas_used();

        Ok(self.accept_tx(trevm, tx))
    }

    /// Accept the state changes and produce a receipt. Push the receipt to the
    /// block.
    ///
    /// This path is used by
    /// - [`TransactionSigned`] objects
    /// - [`Transact`] events
    /// - [`Enter`] events
    fn accept_tx<Db, Insp>(
        &mut self,
        trevm: EvmTransacted<Db, Insp>,
        tx: TransactionSigned,
    ) -> EvmNeedsTx<Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
    {
        // Push the transaction to the block.
        self.processed.push(tx);
        // Accept the result.
        let (result, trevm) = trevm.accept();

        // Create a receipt for the transaction.
        let tx_env = trevm.inner().data.ctx.tx();
        let sender = tx_env.caller;
        // 4844 transactions are not allowed
        let receipt = self.make_receipt(result).into();
        let receipt = if tx_env.gas_priority_fee.is_some() {
            ReceiptEnvelope::Eip1559(receipt)
        } else if !tx_env.access_list.is_empty() {
            ReceiptEnvelope::Eip2930(receipt)
        } else {
            ReceiptEnvelope::Legacy(receipt)
        };

        self.output.push_result(receipt, sender);
        trevm
    }

    /// Execute a transaction.
    ///
    /// This function does the following:
    /// - Recover the signer of the transaction.
    /// - Run the transaction.
    /// - Check the [`AggregateFills`].
    /// - Create a receipt.
    fn execute_transaction<Db, Insp>(
        &mut self,
        mut trevm: EvmNeedsTx<Db, Insp>,
        tx: TransactionSigned,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
    {
        // We set up the span here so that tx details are captured in the event
        // of signature recovery failure.
        let s =
        debug_span!("signet::evm::execute_transaction", tx_hash = %tx.hash(), sender = tracing::field::Empty, nonce = tx.nonce())
                .entered();

        if let Ok(sender) = tx.recover_signer() {
            s.record("sender", sender.to_string());
            // Run the tx, returning from this function if there is a tx error
            let t = run_tx_early_return!(self, trevm, &FillShim(&tx, sender), sender);
            trevm = self.check_fills_and_accept(t, tx)?;
        } else {
            warn!("Failed to recover signer for transaction");
        }
        Ok(trevm)
    }

    /// Execute all transactions. This is run before enters and transacts
    fn execute_all_transactions<Db, Insp>(
        &mut self,
        mut trevm: EvmNeedsTx<Db, Insp>,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
    {
        while !self.to_process.is_empty() {
            let tx = self.to_process.pop_front().expect("checked");
            trevm = self.execute_transaction(trevm, tx)?;
        }

        Ok(trevm)
    }

    /// Credit enters to the recipients. This is done in the middle of the
    /// block, between transactions and transact events.
    fn credit_enters<Insp, Db>(
        &mut self,
        mut trevm: EvmNeedsTx<Db, Insp>,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
    {
        let mut eth_minted = U256::ZERO;
        let mut accts: HashSet<Address> = HashSet::with_capacity(self.extracts.enters.len());

        // Increment the nonce for the minter address by the number of enters.
        // Doing it this way is slightly more efficient than incrementing the
        // nonce in the loop.
        let nonce =
            trevm_try!(trevm.try_read_nonce(MINTER_ADDRESS).map_err(EVMError::Database), trevm);
        trevm_try!(
            trevm
                .try_set_nonce_unchecked(MINTER_ADDRESS, nonce + self.extracts.enters.len() as u64)
                .map_err(EVMError::Database),
            trevm
        );

        for enter in self.extracts.enters.iter() {
            let recipient = enter.recipient();
            let amount = enter.amount();

            // Increase the balance
            trevm_try!(
                trevm.try_increase_balance_unchecked(recipient, amount).map_err(EVMError::Database),
                trevm
            );

            // push receipt and transaction to the block
            self.processed.push(Enter { enter, nonce }.to_reth());
            self.output.push_result(
                ReceiptEnvelope::Eip1559(self.make_enter_receipt().into()),
                MINTER_ADDRESS,
            );

            // Tracking for logging
            accts.insert(recipient);
            eth_minted += amount;
        }

        debug!(
            accounts_touched = accts.len(),
            %eth_minted,
            enters_count = self.extracts.enters.len(),
            "Crediting enters"
        );

        Ok(trevm)
    }

    /// Execute an [`EnterToken`] event.
    ///
    /// [`EnterToken`]: signet_zenith::Passage::EnterToken
    fn execute_enter_token<Db, Insp>(
        &mut self,
        mut trevm: EvmNeedsTx<Db, Insp>,
        idx: usize,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
    {
        let _span = {
            let e = &self.extracts.enter_tokens[idx];
            debug_span!("signet::evm::execute_enter_token", idx, host_tx = %e.tx_hash(), log_index = e.log_index).entered()
        };

        // Get the rollup token address from the host token address.
        let ru_token_addr = self
            .constants
            .rollup_token_from_host_address(self.extracts.enter_tokens[idx].event.token)
            .expect("token enters must be permissioned");

        // Load the nonce as well
        let nonce =
            trevm_try!(trevm.try_read_nonce(MINTER_ADDRESS).map_err(EVMError::Database), trevm);

        let to_execute = EnterToken {
            enter_token: &self.extracts.enter_tokens[idx],
            nonce,
            token: ru_token_addr,
        };

        let t = run_tx_early_return!(self, trevm, &to_execute, MINTER_ADDRESS);
        // No need to check AggregateFills. This call cannot result in orders.
        Ok(self.accept_tx(t, to_execute.to_reth()))
    }

    /// Execute all [`EnterToken`] events.
    fn execute_all_enter_tokens<Db, Insp>(
        &mut self,
        mut trevm: EvmNeedsTx<Db, Insp>,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
    {
        trevm = trevm.try_with_cfg(&DisableGasChecks, |trevm| {
            trevm.try_with_cfg(&DisableNonceCheck, |mut trevm| {
                for i in 0..self.extracts.enter_tokens.len() {
                    trevm = self.execute_enter_token(trevm, i)?;
                }
                Ok(trevm)
            })
        })?;
        Ok(trevm)
    }

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

        let to_execute = Transact { transact: &self.extracts.transacts[idx], nonce };

        let mut t = run_tx_early_return!(self, trevm, &self.extracts.transacts[idx].event, sender);

        {
            // NB: This is a little sensitive.
            // Although the EVM performs a check on the balance of the sender,
            // to ensure they can pay the full price, that check may be
            // invalidated by transaction execution. As a result, we have to
            // perform the same check here, again.
            let transact = &self.extracts.transacts[idx].event;
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

        self.check_fills_and_accept(t, to_execute.to_reth())
    }

    /// Execute all transact events.
    fn execute_all_transacts<Db, Insp>(
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

    /// Clear the balance of the rollup passage. This is run at the end of the
    /// block, after all transactions, enters, and transact events have been
    /// processed. It ensures that ETH sent to the rollup passage is burned,
    /// and before the base fee is credited.
    fn clear_ru_passage_balance<Db, Insp>(
        &self,
        mut trevm: EvmNeedsTx<Db, Insp>,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
    {
        // Zero the balance of the rollup passage (deleting any exited ETH).
        match trevm.try_set_balance_unchecked(self.constants.rollup().passage(), U256::ZERO) {
            Ok(eth_burned) => debug!(%eth_burned, "Zeroed rollup passage balance"),
            Err(e) => return Err(trevm.errored(EVMError::Database(e).into())),
        }
        Ok(trevm)
    }

    /// Credit the base fee to the base fee beneficiary. This is run at the end
    /// of the block, after all transactions, enters, and transact events have
    /// been processed, and after the rollup passage balance has been cleared.
    fn credit_base_fee<Db, Insp>(
        &mut self,
        mut trevm: EvmNeedsTx<Db, Insp>,
        gas_used: u64,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
    {
        // We subtract the fake gas used for enters here. This
        // gives us the gas used for transactions and transact events.
        let base_fee = self.base_fee();
        let amount = U256::from(gas_used) * U256::from(base_fee);

        if amount.is_zero() {
            debug!(%amount, gas_used, base_fee, recipient = %self.base_fee_recipient(), "No base fee to credit");
            return Ok(trevm);
        }

        debug!(%amount, gas_used, base_fee, recipient = %self.base_fee_recipient(), "Crediting base fee");

        trevm_try!(
            trevm
                .try_increase_balance_unchecked(self.base_fee_recipient(), amount)
                .map_err(EVMError::Database),
            trevm
        );

        Ok(trevm)
    }
}

impl trevm::Cfg for SignetDriver<'_, '_> {
    fn fill_cfg_env(&self, cfg_env: &mut CfgEnv) {
        cfg_env.chain_id = self.extracts.chain_id;
    }
}

impl<Db, Insp> BlockDriver<Db, SignetLayered<Insp>> for SignetDriver<'_, '_>
where
    Db: Database + DatabaseCommit,
    Insp: Inspector<Ctx<Db>>,
{
    type Block = Self;

    type Error = SignetDriverError<Db>;

    fn block(&self) -> &Self::Block {
        self
    }

    fn run_txns(&mut self, mut trevm: EvmNeedsTx<Db, Insp>) -> RunTxResult<Self, Db, Insp> {
        let _span = debug_span!(
            "run_txns",
            txn_count = self.to_process.len(),
            enter_count = self.extracts.enters.len(),
            enter_token_count = self.extracts.enter_tokens.len(),
            transact_count = self.extracts.transacts.len(),
            base_fee_beneficiary = %self.constants.base_fee_recipient(),
            rollup_passage = %self.constants.rollup().passage(),
            parent_hash = %self.parent.hash(),
        )
        .entered();

        // NB:
        // The signet block lifecycle is roughly as follows:
        // - Execute the builder-created block by executing each transaction in
        //   order.
        // - Process each enter event in order.
        // - Process each enter token event in order.
        // - Process each transact event in order.
        // - Set the balance of the rollup passage to zero.
        // - Credit the basefee to the basefee beneficiary.

        // Run the transactions.
        // Transaction gas is metered, and pays basefee
        trevm = self.execute_all_transactions(trevm)?;

        // Credit enters to the recipients.
        // Enter gas is unmetered, and does not pay basefee
        trevm = self.credit_enters(trevm)?;

        // Run the enter token events.
        // Enter token gas is unmetered, and does not pay basefee
        trevm = self.execute_all_enter_tokens(trevm)?;

        // Run the transact events.
        // Transact gas is metered, and pays basefee
        trevm = self.execute_all_transacts(trevm)?;

        // Clear the balance of the rollup passage.
        trevm = self.clear_ru_passage_balance(trevm)?;

        // Credit the basefee to the basefee beneficiary. This is the sum
        // of the basefee for the transactions and transact events.
        self.credit_base_fee(trevm, self.payable_gas_used())
    }

    fn post_block(&mut self, _trevm: &crate::EvmNeedsBlock<Db, Insp>) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl trevm::Block for SignetDriver<'_, '_> {
    fn fill_block_env(&self, block_env: &mut BlockEnv) {
        let BlockEnv {
            number,
            beneficiary,
            timestamp,
            gas_limit,
            basefee,
            difficulty,
            prevrandao,
            blob_excess_gas_and_price,
        } = block_env;
        *number = self.ru_height();
        *beneficiary = self.beneficiary();
        *timestamp = self.extracts.host_block.timestamp;
        *gas_limit = self.gas_limit();
        *basefee = self.base_fee();
        *difficulty = self.extracts.host_block.difficulty;
        *prevrandao = Some(self.extracts.host_block.mix_hash);
        *blob_excess_gas_and_price =
            Some(BlobExcessGasAndPrice { excess_blob_gas: 0, blob_gasprice: 0 });
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_utils::*;
    use alloy::{
        consensus::{
            constants::{ETH_TO_WEI, GWEI_TO_WEI},
            SignableTransaction, TxEip1559,
        },
        primitives::{Sealable, B256, B64},
        signers::{local::PrivateKeySigner, SignerSync},
    };
    use reth::primitives::{Block, RecoveredBlock, Transaction};
    use signet_extract::ExtractedEvent;
    use signet_types::{
        config::{HostConfig, PredeployTokens, RollupConfig},
        test_utils::*,
    };
    use trevm::revm::database::in_memory_db::InMemoryDB;

    /// Make a fake block with a specific number.
    pub(super) fn fake_block(number: u64) -> RecoveredBlock<Block> {
        let header = Header {
            difficulty: U256::from(0x4000_0000),
            number,
            mix_hash: B256::repeat_byte(0xed),
            nonce: B64::repeat_byte(0xbe),
            timestamp: 1716555586, // the time when i wrote this function lol
            excess_blob_gas: Some(0),
            ..Default::default()
        };
        let (header, hash) = header.seal_slow().into_parts();
        RecoveredBlock::new(
            Block::new(
                header,
                BlockBody { transactions: vec![], ommers: vec![], withdrawals: None },
            ),
            vec![],
            hash,
        )
    }

    /// Make a simple send transaction.
    pub(super) fn simple_send(
        to: Address,
        amount: U256,
        nonce: u64,
    ) -> reth::primitives::Transaction {
        TxEip1559 {
            nonce,
            gas_limit: 21_000,
            to: alloy::primitives::TxKind::Call(to),
            value: amount,
            chain_id: TEST_RU_CHAIN_ID,
            max_fee_per_gas: GWEI_TO_WEI as u128 * 100,
            max_priority_fee_per_gas: GWEI_TO_WEI as u128,
            ..Default::default()
        }
        .into()
    }

    /// Sign a transaction with a wallet.
    pub(super) fn sign_tx_with_key_pair(
        wallet: &PrivateKeySigner,
        tx: Transaction,
    ) -> TransactionSigned {
        let signature = wallet.sign_hash_sync(&tx.signature_hash()).unwrap();
        TransactionSigned::new_unhashed(tx, signature)
    }

    /// Make a wallet with a deterministic keypair.
    pub(super) fn make_wallet(i: u8) -> PrivateKeySigner {
        PrivateKeySigner::from_bytes(&B256::repeat_byte(i)).unwrap()
    }

    struct TestEnv {
        pub wallets: Vec<PrivateKeySigner>,
        pub nonces: [u64; 10],
        pub sequence: u64,
    }

    impl TestEnv {
        fn new() -> Self {
            let wallets = (1..=10).map(make_wallet).collect::<Vec<_>>();

            Self { wallets, nonces: [0; 10], sequence: 1 }
        }

        fn driver<'a, 'b>(
            &self,
            extracts: &'a mut Extracts<'b>,
            txns: Vec<TransactionSigned>,
        ) -> SignetDriver<'a, 'b> {
            let (header, hash) =
                Header { gas_limit: 30_000_000, ..Default::default() }.seal_slow().into_parts();
            SignetDriver::new(
                extracts,
                txns.into(),
                SealedHeader::new(header, hash),
                SignetSystemConstants::new(
                    HostConfig::new(
                        1,
                        0,
                        Address::repeat_byte(0xdd),
                        Address::repeat_byte(0xee),
                        Address::repeat_byte(0xff),
                        Address::repeat_byte(0x66),
                        PredeployTokens::new(
                            Address::repeat_byte(0xba),
                            Address::repeat_byte(0xcb),
                            Address::repeat_byte(0xdc),
                        ),
                    ),
                    RollupConfig::new(
                        TEST_RU_CHAIN_ID,
                        Address::repeat_byte(0xff),
                        Address::repeat_byte(0),
                        Address::repeat_byte(1),
                        PredeployTokens::new(
                            Address::repeat_byte(0xaa),
                            Address::repeat_byte(0xbb),
                            Address::repeat_byte(0xcc),
                        ),
                    ),
                ),
            )
        }

        fn trevm(&self) -> crate::EvmNeedsBlock<InMemoryDB> {
            let mut trevm = test_signet_evm();
            for wallet in &self.wallets {
                let address = wallet.address();
                trevm.test_set_balance(address, U256::from(ETH_TO_WEI * 100));
            }
            trevm
        }

        /// Get the next zenith header in the sequence
        fn next_block(&mut self) -> RecoveredBlock<Block> {
            let block = fake_block(self.sequence);
            self.sequence += 1;
            block
        }

        fn signed_simple_send(
            &mut self,
            from: usize,
            to: Address,
            amount: U256,
        ) -> TransactionSigned {
            let wallet = &self.wallets[from];
            let tx = simple_send(to, amount, self.nonces[from]);
            let tx = sign_tx_with_key_pair(wallet, tx);
            self.nonces[from] += 1;
            tx
        }
    }

    #[test]
    fn test_simple_send() {
        let mut context = TestEnv::new();

        // Set up a simple transfer
        let to = Address::repeat_byte(2);
        let tx = context.signed_simple_send(0, to, U256::from(100));

        // Setup the driver
        let block = context.next_block();
        let mut extracts = Extracts::empty(&block);
        let mut driver = context.driver(&mut extracts, vec![tx.clone()]);

        // Run the EVM
        let mut trevm = context.trevm().drive_block(&mut driver).unwrap();
        let (sealed_block, receipts) = driver.finish();

        // Assert that the EVM balance increased
        assert_eq!(sealed_block.senders().len(), 1);
        assert_eq!(sealed_block.body().transactions().next(), Some(&tx));
        assert_eq!(receipts.len(), 1);

        assert_eq!(trevm.read_balance(to), U256::from(100));
    }

    #[test]
    fn test_two_sends() {
        let mut context = TestEnv::new();

        // Set up a simple transfer
        let to = Address::repeat_byte(2);
        let tx1 = context.signed_simple_send(0, to, U256::from(100));

        let to2 = Address::repeat_byte(3);
        let tx2 = context.signed_simple_send(0, to2, U256::from(100));

        // Setup the driver
        let block = context.next_block();
        let mut extracts = Extracts::empty(&block);
        let mut driver = context.driver(&mut extracts, vec![tx1.clone(), tx2.clone()]);

        // Run the EVM
        let mut trevm = context.trevm().drive_block(&mut driver).unwrap();
        let (sealed_block, receipts) = driver.finish();

        // Assert that the EVM balance increased
        assert_eq!(sealed_block.senders().len(), 2);
        assert_eq!(sealed_block.body().transactions().collect::<Vec<_>>(), vec![&tx1, &tx2]);
        assert_eq!(receipts.len(), 2);

        assert_eq!(trevm.read_balance(to), U256::from(100));
        assert_eq!(trevm.read_balance(to2), U256::from(100));
    }

    #[test]
    fn test_execute_two_blocks() {
        let mut context = TestEnv::new();
        let sender = context.wallets[0].address();

        let to = Address::repeat_byte(2);
        let tx = context.signed_simple_send(0, to, U256::from(100));

        // Setup the driver
        let block = context.next_block();
        let mut extracts = Extracts::empty(&block);
        let mut driver = context.driver(&mut extracts, vec![tx.clone()]);

        // Run the EVM
        let mut trevm = context.trevm().drive_block(&mut driver).unwrap();
        let (sealed_block, receipts) = driver.finish();

        assert_eq!(sealed_block.senders().len(), 1);
        assert_eq!(sealed_block.body().transactions().collect::<Vec<_>>(), vec![&tx]);
        assert_eq!(receipts.len(), 1);
        assert_eq!(trevm.read_balance(to), U256::from(100));
        assert_eq!(trevm.read_nonce(sender), 1);

        // Repeat the above for the next block
        // same recipient
        let tx = context.signed_simple_send(0, to, U256::from(100));

        // Setup the driver
        let block = context.next_block();
        let mut extracts = Extracts::empty(&block);
        let mut driver = context.driver(&mut extracts, vec![tx.clone()]);

        // Run the EVM
        let mut trevm = trevm.drive_block(&mut driver).unwrap();
        let (sealed_block, receipts) = driver.finish();

        assert_eq!(sealed_block.senders().len(), 1);
        assert_eq!(sealed_block.body().transactions().collect::<Vec<_>>(), vec![&tx]);
        assert_eq!(receipts.len(), 1);
        assert_eq!(trevm.read_balance(to), U256::from(200));
    }

    #[test]
    fn test_an_enter() {
        let mut context = TestEnv::new();
        let user = Address::repeat_byte(2);

        // Set up a fake event
        let fake_tx = TransactionSigned::default();
        let fake_receipt: reth::primitives::Receipt = Default::default();

        let enter = signet_zenith::Passage::Enter {
            rollupChainId: U256::from(TEST_RU_CHAIN_ID),
            rollupRecipient: user,
            amount: U256::from(100),
        };

        // Setup the driver
        let block = context.next_block();
        let mut extracts = Extracts::empty(&block);
        extracts.enters.push(ExtractedEvent {
            tx: &fake_tx,
            receipt: &fake_receipt,
            log_index: 0,
            event: enter,
        });
        let mut driver = context.driver(&mut extracts, vec![]);

        // Run the EVM
        let mut trevm = context.trevm().drive_block(&mut driver).unwrap();
        let (sealed_block, receipts) = driver.finish();

        assert_eq!(sealed_block.senders().len(), 1);
        assert_eq!(
            sealed_block.body().transactions().collect::<Vec<_>>(),
            vec![&Enter { enter: &extracts.enters[0], nonce: 0 }.to_reth()]
        );
        assert_eq!(receipts.len(), 1);
        assert_eq!(trevm.read_balance(user), U256::from(100));
        assert_eq!(trevm.read_nonce(user), 0);
    }

    #[test]
    fn test_a_transact() {
        let mut context = TestEnv::new();
        let sender = Address::repeat_byte(1);
        let recipient = Address::repeat_byte(2);

        // Set up a couple fake events
        let fake_tx = TransactionSigned::default();
        let fake_receipt: reth::primitives::Receipt = Default::default();

        let enter = signet_zenith::Passage::Enter {
            rollupChainId: U256::from(TEST_RU_CHAIN_ID),
            rollupRecipient: sender,
            amount: U256::from(ETH_TO_WEI),
        };

        let transact = signet_zenith::Transactor::Transact {
            rollupChainId: U256::from(TEST_RU_CHAIN_ID),
            sender,
            to: recipient,
            data: Default::default(),
            value: U256::from(100),
            gas: U256::from(21_000),
            maxFeePerGas: U256::from(GWEI_TO_WEI),
        };

        // Setup the driver
        let block = context.next_block();
        let mut extracts = Extracts::empty(&block);
        extracts.enters.push(ExtractedEvent {
            tx: &fake_tx,
            receipt: &fake_receipt,
            log_index: 0,
            event: enter,
        });
        extracts.transacts.push(ExtractedEvent {
            tx: &fake_tx,
            receipt: &fake_receipt,
            log_index: 0,
            event: transact,
        });

        let mut driver = context.driver(&mut extracts, vec![]);

        // Run the EVM
        let mut trevm = context.trevm().drive_block(&mut driver).unwrap();
        let (sealed_block, receipts) = driver.finish();

        assert_eq!(sealed_block.senders(), vec![MINTER_ADDRESS, sender]);
        assert_eq!(
            sealed_block.body().transactions().collect::<Vec<_>>(),
            vec![
                &Enter { enter: &extracts.enters[0], nonce: 0 }.to_reth(),
                &Transact { transact: &extracts.transacts[0], nonce: 0 }.to_reth()
            ]
        );
        assert_eq!(receipts.len(), 2);
        assert_eq!(trevm.read_balance(recipient), U256::from(100));
    }
}
