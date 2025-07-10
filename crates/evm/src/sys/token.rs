use std::sync::OnceLock;

use crate::sys::{MintTokenSysLog, SysBase, SysTx, UnmeteredSysTx};
use alloy::{
    consensus::{TxEip1559, TxReceipt},
    primitives::{Address, Bytes, Log, TxKind, U256},
    sol_types::SolCall,
};
use signet_extract::ExtractedEvent;
use signet_types::{
    constants::MINTER_ADDRESS,
    primitives::{Transaction, TransactionSigned},
    MagicSig,
};
use signet_zenith::Passage;
use trevm::{
    revm::context::{TransactTo, TransactionType, TxEnv},
    MIN_TRANSACTION_GAS,
};

/// System transaction to mint tokens.
#[derive(Debug, Clone)]
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
    nonce: Option<u64>,
    /// The rollup chain ID.
    rollup_chain_id: u64,

    /// The ABI-encoded call for the mint operation./s
    encoded_call: OnceLock<Bytes>,
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
        *data = self.encoded_call().clone();
        *nonce = self.nonce.expect("must be set");
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
        token: Address,
        event: &ExtractedEvent<'_, R, Passage::EnterToken>,
    ) -> Self {
        Self {
            recipient: event.event.recipient(),
            amount: event.event.amount(),
            token,
            host_token: event.event.token,
            magic_sig: event.magic_sig(),
            nonce: None,
            rollup_chain_id: event.rollup_chain_id(),
            encoded_call: OnceLock::new(),
        }
    }

    /// Create a new [`MintToken`] instance from an [`ExtractedEvent`]
    /// containing a [`Passage::Enter`] event.
    pub fn from_enter<R: TxReceipt<Log = Log>>(
        token: Address,
        event: &ExtractedEvent<'_, R, Passage::Enter>,
    ) -> Self {
        Self {
            recipient: event.event.recipient(),
            amount: event.event.amount(),
            token,
            host_token: Address::repeat_byte(0xee),
            magic_sig: event.magic_sig(),
            nonce: None,
            rollup_chain_id: event.rollup_chain_id(),
            encoded_call: OnceLock::new(),
        }
    }

    /// Create the ABI-encoded call for the mint operation.
    pub const fn mint_call(&self) -> signet_zenith::mintCall {
        signet_zenith::mintCall { amount: self.amount, to: self.recipient }
    }

    /// Get the ABI-encoded call for the mint operation, lazily initialized.
    pub fn encoded_call(&self) -> &Bytes {
        self.encoded_call.get_or_init(|| self.mint_call().abi_encode().into())
    }

    /// Create a new [`Log`] for the [`MintToken`] operation.
    const fn make_sys_log(&self) -> MintTokenSysLog {
        MintTokenSysLog {
            txHash: self.magic_sig.txid,
            logIndex: self.magic_sig.event_idx as u64,
            recipient: self.recipient,
            amount: self.amount,
            hostToken: self.host_token,
        }
    }

    /// Convert the [`MintToken`] instance into a [`TransactionSigned`].
    fn make_transaction(&self) -> TransactionSigned {
        let input = self.encoded_call().clone();

        TransactionSigned::new_unhashed(
            Transaction::Eip1559(TxEip1559 {
                chain_id: self.rollup_chain_id,
                nonce: self.nonce.expect("must be set"),
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

impl SysBase for MintToken {
    fn populate_nonce(&mut self, nonce: u64) {
        self.nonce = Some(nonce);
    }

    fn produce_transaction(&self) -> TransactionSigned {
        self.make_transaction()
    }

    fn produce_log(&self) -> Log {
        self.make_sys_log().into()
    }

    fn evm_sender(&self) -> Address {
        MINTER_ADDRESS
    }

    fn has_nonce(&self) -> bool {
        self.nonce.is_some()
    }
}

impl SysTx for MintToken {
    fn callee(&self) -> TxKind {
        self.token.into()
    }

    fn input(&self) -> Bytes {
        self.encoded_call().clone()
    }
}

impl UnmeteredSysTx for MintToken {}
