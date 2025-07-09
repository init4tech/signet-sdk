use crate::{
    orders::SignetInspector, BlockResult, EvmNeedsTx, EvmTransacted, ExecutionOutcome, RunTxResult,
    SignetLayered, BASE_GAS,
};
use alloy::{
    consensus::{
        transaction::SignerRecoverable, BlockHeader, Header, ReceiptEnvelope, Transaction as _,
        TxType,
    },
    eips::eip1559::{BaseFeeParams, INITIAL_BASE_FEE as EIP1559_INITIAL_BASE_FEE},
    primitives::{Address, Bloom, U256},
};
use signet_extract::{Extractable, ExtractedEvent, Extracts};
use signet_types::{
    constants::SignetSystemConstants,
    primitives::{BlockBody, RecoveredBlock, SealedBlock, SealedHeader, TransactionSigned},
    AggregateFills, MarketError,
};
use signet_zenith::{Passage, Transactor, MINTER_ADDRESS};
use std::collections::{HashSet, VecDeque};
use tracing::{debug, debug_span, warn};
use trevm::{
    fillers::{DisableGasChecks, DisableNonceCheck},
    helpers::Ctx,
    revm::{
        context::{
            result::{EVMError, ExecutionResult},
            BlockEnv, CfgEnv, ContextTr, TransactTo, TxEnv,
        },
        context_interface::block::BlobExcessGasAndPrice,
        database::State,
        Database, DatabaseCommit, Inspector,
    },
    trevm_try, BlockDriver, BlockOutput, Tx,
};

macro_rules! run_tx {
    ($self:ident, $trevm:ident, $tx:expr, $sender:expr) => {{
        let trevm = $trevm.fill_tx($tx);

        let _guard = tracing::trace_span!("run_tx", block_env = ?trevm.block(), tx = ?$tx, tx_env = ?trevm.tx(), spec_id = ?trevm.spec_id()).entered();

        match trevm.run() {
            Ok(t) => {
                tracing::debug!("evm executed successfully");
                ControlFlow::Keep(t)
            },
            Err(e) => {
                if e.is_transaction_error() {
                    tracing::debug!(
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

/// Shim to impl [`Tx`] for [`Passage::EnterToken`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EnterTokenFiller<'a, 'b, R> {
    /// The extracted event for the enter token event.
    pub enter_token: &'a ExtractedEvent<'b, R, Passage::EnterToken>,
    /// The nonce of the transaction.
    pub nonce: u64,
    /// The address of the token being minted.
    pub token: Address,
}

impl<R: Sync> Tx for EnterTokenFiller<'_, '_, R> {
    fn fill_tx_env(&self, tx_env: &mut TxEnv) {
        self.enter_token.event.fill_tx_env(tx_env);
        tx_env.kind = TransactTo::Call(self.token);
        tx_env.nonce = self.nonce;
    }
}

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

        match self.0 {
            TransactionSigned::Legacy(tx) => {
                let tx = tx.tx();
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
            TransactionSigned::Eip2930(tx) => {
                let tx = tx.tx();
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
            TransactionSigned::Eip1559(tx) => {
                let tx = tx.tx();
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
            TransactionSigned::Eip4844(tx) => {
                let tx = tx.tx();
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
            TransactionSigned::Eip7702(tx) => {
                let tx = tx.tx();
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
                authorization_list.clone_from(
                    &tx.authorization_list
                        .iter()
                        .cloned()
                        .map(alloy::signers::Either::Left)
                        .collect(),
                );
            }
        }
    }
}

/// A driver for the Signet EVM
#[derive(Debug)]
pub struct SignetDriver<'a, 'b, C: Extractable> {
    /// The block extracts.
    extracts: &'a Extracts<'b, C>,

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

impl<'a, 'b, C: Extractable> SignetDriver<'a, 'b, C> {
    /// Create a new driver.
    pub fn new(
        extracts: &'a Extracts<'b, C>,
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
    pub const fn extracts(&self) -> &Extracts<'b, C> {
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
        self.extracts.ru_header().map(|h| h.rewardAddress).unwrap_or(self.parent.beneficiary())
    }

    /// Base fee beneficiary of the current block.
    pub const fn base_fee_recipient(&self) -> Address {
        self.constants.base_fee_recipient()
    }

    /// Gas limit of the current block.
    pub fn gas_limit(&self) -> u64 {
        self.extracts.ru_header().map(|h| h.gas_limit()).unwrap_or(self.parent.gas_limit())
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
    pub fn finish(self) -> (RecoveredBlock, Vec<ReceiptEnvelope>) {
        let header = self.construct_sealed_header();
        let (receipts, senders, _) = self.output.into_parts();

        let body = BlockBody { transactions: self.processed, ommers: vec![], withdrawals: None };
        let block = SealedBlock { header, body };
        let block = RecoveredBlock::new(block, senders);

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
            execution_outcome: ExecutionOutcome::new(trevm.finish(), vec![receipts], ru_height),
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
    fn make_enter_receipt(
        &self,
        enter: &ExtractedEvent<'_, C::Receipt, Passage::Enter>,
    ) -> alloy::consensus::Receipt {
        let cumulative_gas_used = self.cumulative_gas_used().saturating_add(BASE_GAS as u64);

        let sys_log = crate::sys_log::Enter::from(enter).into();

        alloy::consensus::Receipt { status: true.into(), cumulative_gas_used, logs: vec![sys_log] }
    }

    /// Construct a block header for DB and evm execution.
    fn construct_header(&self) -> Header {
        Header {
            parent_hash: self.parent.hash(),
            number: self.ru_height(),
            gas_limit: self.gas_limit(),
            timestamp: self.extracts.host_block.timestamp(),
            base_fee_per_gas: Some(self.base_fee()),
            beneficiary: self.beneficiary(),

            logs_bloom: self.logs_bloom(),
            gas_used: self.cumulative_gas_used(),

            difficulty: self.extracts.host_block.difficulty(),

            mix_hash: self.extracts.host_block.mix_hash().unwrap_or_default(),
            nonce: self.extracts.host_block.nonce().unwrap_or_default(),
            parent_beacon_block_root: self.extracts.host_block.parent_beacon_block_root(),

            ..Default::default()
        }
    }

    /// Construct a sealed header for DB and evm execution.
    fn construct_sealed_header(&self) -> SealedHeader {
        let header = self.construct_header();
        SealedHeader::new(header)
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
        extract: Option<&ExtractedEvent<'_, C::Receipt, Transactor::Transact>>,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
    {
        // Taking these clears the context for reuse.
        let (agg_orders, agg_fills) =
            trevm.inner_mut_unchecked().inspector.as_mut_detector().take_aggregates();

        // We check the AggregateFills here, and if it fails, we discard the
        // transaction outcome and push a failure receipt.
        if let Err(err) = self.working_context.checked_remove_ru_tx_events(&agg_orders, &agg_fills)
        {
            debug!(%err, "Discarding transaction outcome due to market error");
            return Ok(trevm.reject());
        }

        if let ExecutionResult::Success { logs, .. } = trevm.result_mut_unchecked() {
            if let Some(extract) = extract {
                // Push the sys_log to the outcome
                let sys_log = crate::sys_log::Transact::from(extract).into();
                logs.push(sys_log);
            }
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
        let tx_env = trevm.inner().ctx.tx();
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
            trevm = self.check_fills_and_accept(t, tx, None)?;
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
            self.processed.push(enter.make_transaction(nonce));
            self.output.push_result(
                ReceiptEnvelope::Eip1559(self.make_enter_receipt(enter).into()),
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

        let extract = &self.extracts.enter_tokens[idx];

        let filler = EnterTokenFiller { enter_token: extract, nonce, token: ru_token_addr };
        let mut t = run_tx_early_return!(self, trevm, &filler, MINTER_ADDRESS);

        // push a sys_log to the outcome
        if let ExecutionResult::Success { logs, .. } = t.result_mut_unchecked() {
            let sys_log = crate::sys_log::EnterToken::from(extract).into();
            logs.push(sys_log)
        }
        let tx = extract.make_transaction(nonce, ru_token_addr);

        // No need to check AggregateFills. This call cannot result in orders.
        Ok(self.accept_tx(t, tx))
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

impl<C: Extractable> trevm::Cfg for SignetDriver<'_, '_, C> {
    fn fill_cfg_env(&self, cfg_env: &mut CfgEnv) {
        cfg_env.chain_id = self.extracts.chain_id;
    }
}

impl<Db, Insp, C: Extractable> BlockDriver<Db, SignetLayered<Insp>> for SignetDriver<'_, '_, C>
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

impl<C: Extractable> trevm::Block for SignetDriver<'_, '_, C> {
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
        *number = U256::from(self.ru_height());
        *beneficiary = self.beneficiary();
        *timestamp = U256::from(self.extracts.host_block.timestamp());
        *gas_limit = self.gas_limit();
        *basefee = self.base_fee();
        *difficulty = self.extracts.host_block.difficulty();
        *prevrandao = self.extracts.host_block.mix_hash();
        *blob_excess_gas_and_price =
            Some(BlobExcessGasAndPrice { excess_blob_gas: 0, blob_gasprice: 0 });
    }
}
