use crate::{
    orders::SignetInspector, BlockResult, EvmNeedsTx, EvmTransacted, ExecutionOutcome, RunTxResult,
    SignetLayered,
};
use alloy::{
    consensus::{
        transaction::SignerRecoverable, BlockHeader, Header, ReceiptEnvelope, Transaction as _,
        TxType,
    },
    eips::eip1559::{BaseFeeParams, INITIAL_BASE_FEE as EIP1559_INITIAL_BASE_FEE},
    primitives::{map::HashSet, Address, Bloom, U256},
};
use signet_extract::{Extractable, Extracts};
use signet_types::{
    constants::SignetSystemConstants,
    primitives::{BlockBody, RecoveredBlock, SealedBlock, SealedHeader, TransactionSigned},
    AggregateFills, MarketError,
};
use std::collections::VecDeque;
use tracing::{debug, debug_span, info_span, warn};
use trevm::{
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
    trevm_try, Block, BlockDriver, BlockOutput, Cfg, Tx,
};

/// Used internally to signal that the transaction should be discarded.
pub(crate) enum ControlFlow<Db, Insp>
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
    pub(crate) extracts: &'a Extracts<'b, C>,

    /// Set of addresses that generated transact events and should be aliased
    /// because they contain code.
    pub(crate) to_alias: HashSet<Address>,

    /// Parent rollup block.
    parent: SealedHeader,

    /// Rollup constants, including pre-deploys
    pub(crate) constants: SignetSystemConstants,

    /// The working context is a clone of the block's [`AggregateFills`] that
    /// is updated progessively as the block is evaluated.
    working_context: AggregateFills,

    /// Transactions in the RU block (if any)
    to_process: VecDeque<TransactionSigned>,

    /// Transactions that have been processed.
    pub(crate) processed: Vec<TransactionSigned>,

    /// Receipts and senders.
    pub(crate) output: BlockOutput,

    /// Payable gas used in the block.
    payable_gas_used: u64,
}

impl<'a, 'b, C: Extractable> SignetDriver<'a, 'b, C> {
    /// Create a new driver.
    pub fn new(
        extracts: &'a Extracts<'b, C>,
        to_alias: HashSet<Address>,
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
            to_alias,
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
        let host_height = self.extracts.host_block.number();
        let (sealed_block, receipts) = self.finish();
        BlockResult {
            host_height,
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
    /// - [`Transactor::Transact`] events
    pub(crate) fn check_fills_and_accept<Db, Insp>(
        &mut self,
        mut trevm: EvmTransacted<Db, Insp>,
        tx: TransactionSigned,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
    {
        // Taking these clears the context for reuse.
        let (agg_fills, agg_orders) =
            trevm.inner_mut_unchecked().inspector.as_mut_detector().take_aggregates();

        // We check the AggregateFills here, and if it fails, we discard the
        // transaction outcome and push a failure receipt.
        if let Err(err) = self.working_context.checked_remove_ru_tx_events(&agg_fills, &agg_orders)
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
    /// - [`Transactor::Transact`] events
    /// - [`Passage::Enter`] events
    ///
    /// [`Passage::Enter`]: signet_zenith::Passage::Enter
    pub(crate) fn accept_tx<Db, Insp>(
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
        while let Some(tx) = self.to_process.pop_front() {
            if tx.is_eip4844() {
                warn!("EIP-4844 transactions are not allowed in Signet blocks");
                continue;
            }
            trevm = self.execute_transaction(trevm, tx)?;
        }

        Ok(trevm)
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

impl<C: Extractable> Cfg for SignetDriver<'_, '_, C> {
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
        let _span = info_span!(
            "SignetDriver::run_txns",
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
        // - Run system built-in application logic
        // - Process each transact event in order.
        // - Set the balance of the rollup passage to zero.
        // - Credit the basefee to the basefee beneficiary.

        // Run the transactions.
        // Transaction gas is metered, and pays basefee
        trevm = self.execute_all_transactions(trevm)?;

        // Run all Enter and EnterToken events
        // Gas is unmetered, and does not pay basefee
        trevm = self.run_all_mints(trevm)?;

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

impl<C: Extractable> Block for SignetDriver<'_, '_, C> {
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
