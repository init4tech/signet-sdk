use crate::{sys::MintTokenSysLog, ControlFlow, EvmNeedsTx, RunTxResult, SignetDriver};
use alloy::{
    consensus::{TxEip1559, TxReceipt},
    primitives::{Address, Log, U256},
    sol_types::SolCall,
};
use signet_extract::{Extractable, ExtractedEvent};
use signet_types::{
    constants::MINTER_ADDRESS,
    primitives::{Transaction, TransactionSigned},
    MagicSig,
};
use signet_zenith::Passage;
use tracing::debug_span;
use trevm::{
    helpers::Ctx,
    revm::{
        context::{result::ExecutionResult, TransactTo, TransactionType, TxEnv},
        Database, DatabaseCommit, Inspector,
    },
    MIN_TRANSACTION_GAS,
};

/// System transaction to mint tokens.
#[derive(Debug, Clone, Copy)]
pub struct MintToken {
    /// The address that will receive the minted tokens.
    recipient: Address,
    /// The amount of tokens to mint.
    amount: U256,
    /// The token being minted.
    token: Address,
    /// The corresponding token on the host.
    host_token: Address,

    /// The magic signature for the mint.
    magic_sig: MagicSig,

    /// The nonce of the mint transaction.
    nonce: u64,
    /// The rollup chain ID.
    rollup_chain_id: u64,
}

impl trevm::Tx for MintToken {
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

        *tx_type = TransactionType::Custom as u8;
        *caller = MINTER_ADDRESS;
        *gas_limit = 1_000_000;
        *gas_price = 0;
        *kind = TransactTo::Call(self.token);
        *value = U256::ZERO;
        *data = self.mint_call().abi_encode().into();
        *nonce = self.nonce;
        *chain_id = Some(self.rollup_chain_id);
        *access_list = Default::default();
        *gas_priority_fee = Some(0);
        blob_hashes.clear();
        *max_fee_per_blob_gas = 0;
        authorization_list.clear();
    }
}

impl MintToken {
    /// Create a new [`MintToken`] instance from an [`ExtractedEvent`]
    /// containing a [`Passage::EnterToken`] event.
    pub fn from_enter_token<R: TxReceipt<Log = Log>>(
        nonce: u64,
        token: Address,
        event: &ExtractedEvent<'_, R, Passage::EnterToken>,
    ) -> Self {
        Self {
            recipient: event.event.recipient(),
            amount: event.event.amount(),
            token,
            host_token: event.event.token,
            magic_sig: event.magic_sig(),
            nonce,
            rollup_chain_id: event.rollup_chain_id(),
        }
    }

    /// Create a new [`MintToken`] instance from an [`ExtractedEvent`]
    /// containing a [`Passage::Enter`] event.
    pub fn from_enter<R: TxReceipt<Log = Log>>(
        nonce: u64,
        token: Address,
        event: &ExtractedEvent<'_, R, Passage::Enter>,
    ) -> Self {
        Self {
            recipient: event.event.recipient(),
            amount: event.event.amount(),
            token,
            host_token: Address::repeat_byte(0xee),
            magic_sig: event.magic_sig(),
            nonce,
            rollup_chain_id: event.rollup_chain_id(),
        }
    }

    /// Create the ABI-encoded call for the mint operation.
    const fn mint_call(&self) -> signet_zenith::mintCall {
        signet_zenith::mintCall { amount: self.amount, to: self.recipient }
    }

    /// Create a new [`Log`] for the [`MintToken`] operation.
    const fn to_log(self) -> MintTokenSysLog {
        MintTokenSysLog {
            txHash: self.magic_sig.txid,
            logIndex: self.magic_sig.event_idx as u64,
            recipient: self.recipient,
            amount: self.amount,
            hostToken: self.host_token, // TODO: this needs to be the HOST token
        }
    }

    /// Convert the [`MintToken`] instance into a [`TransactionSigned`].
    pub fn to_transaction(self) -> TransactionSigned {
        let input = self.mint_call().abi_encode().into();

        TransactionSigned::new_unhashed(
            Transaction::Eip1559(TxEip1559 {
                chain_id: self.rollup_chain_id,
                nonce: self.nonce,
                gas_limit: MIN_TRANSACTION_GAS,
                max_fee_per_gas: 0,
                max_priority_fee_per_gas: 0,
                // NB: set to the address of the token contract.
                to: self.token.into(),
                value: U256::ZERO,
                access_list: Default::default(),
                input, // NB: set to the ABI-encoded input for the `mint` function, which dictates the amount and recipient.
            }),
            self.magic_sig.into(),
        )
    }
}

impl<'a, 'b, C> SignetDriver<'a, 'b, C>
where
    C: Extractable,
{
    /// Execute a [`MintToken`], triggered by either a [`Passage::Enter`] or a
    /// [`Passage::EnterToken`].
    pub(crate) fn execute_mint_token<Db, Insp>(
        &mut self,
        trevm: EvmNeedsTx<Db, Insp>,
        mint: &MintToken,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
    {
        let _span = debug_span!("signet::evm::Execute_mint_token", host_tx = %mint.magic_sig.txid, log_index = mint.magic_sig.event_idx).entered();

        // Run the transaction.
        let mut t = run_tx_early_return!(self, trevm, mint, MINTER_ADDRESS);

        // push a sys_log to the outcome
        if let ExecutionResult::Success { logs, .. } = t.result_mut_unchecked() {
            logs.push(mint.to_log().into());
        }

        // No need to check AggregateFills. This call cannot result in orders.
        let tx = mint.to_transaction();
        Ok(self.accept_tx(t, tx))
    }
}
