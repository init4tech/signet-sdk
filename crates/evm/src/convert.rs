//! Bespoke conversion utilities for converting between alloy and reth types.

use alloy::{
    consensus::{ReceiptEnvelope, TxEip1559},
    primitives::{Address, PrimitiveSignature as Signature, U256},
    sol_types::SolCall,
};
use reth::primitives::{Transaction, TransactionSigned};
use signet_extract::ExtractedEvent;
use signet_types::{MagicSig, MagicSigInfo};
use trevm::{
    revm::primitives::{TransactTo, TxEnv},
    Tx,
};
use zenith_types::{Passage, Transactor};

/// This is the default minimum gas cost for a transaction, used by Ethereum
/// for simple sends to accounts without code.
pub(crate) const BASE_TX_GAS_COST: u64 = 21_000;

/// Utility trait to convert a type to a Reth primitive type.
/// This is used mainly where we need to convert to a reth primitive type
/// because reth does not support the alloy equivalents.
pub trait ToRethPrimitive {
    /// The Reth primitive type that the type can be converted to.
    type RethPrimitive;

    /// Convert the type to a Reth primitive type.
    fn to_reth(self) -> Self::RethPrimitive;
}

/// Contains information necessary to produce a [`TransactionSigned`] for the
/// extracted [`Transactor::Transact`] event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Transact<'a, 'b> {
    /// The extracted event for the transact event.
    pub transact: &'a ExtractedEvent<'b, Transactor::Transact>,
    /// The nonce of the transaction.
    pub nonce: u64,
}

impl Transact<'_, '_> {
    /// Get the magic signature for the transact event, containing sender
    /// information.
    pub(crate) fn magic_sig(&self) -> MagicSig {
        MagicSig {
            ty: MagicSigInfo::Transact { sender: self.transact.sender() },
            txid: self.transact.tx_hash(),
            event_idx: self.transact.log_index,
        }
    }

    /// Get the reth transaction signature for the transact event.
    pub(crate) fn signature(&self) -> Signature {
        self.magic_sig().into()
    }
}

impl ToRethPrimitive for Transact<'_, '_> {
    type RethPrimitive = TransactionSigned;

    fn to_reth(self) -> Self::RethPrimitive {
        TransactionSigned::new_unhashed(
            Transaction::Eip1559(TxEip1559 {
                chain_id: self.transact.rollup_chain_id(),
                nonce: self.nonce,
                gas_limit: self.transact.gas.to::<u64>(),
                max_fee_per_gas: self.transact.maxFeePerGas.to::<u128>(),
                max_priority_fee_per_gas: 0,
                to: self.transact.to.into(),
                value: self.transact.value,
                access_list: Default::default(),
                input: self.transact.data.clone(),
            }),
            self.signature(),
        )
    }
}

/// Contains information necessary to produce a [`TransactionSigned`] for the
/// extracted [`Passage::Enter`] event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Enter<'a, 'b> {
    /// The extracted event for the enter event.
    pub enter: &'a ExtractedEvent<'b, Passage::Enter>,
    /// The nonce of the transaction.
    pub nonce: u64,
}

impl Enter<'_, '_> {
    /// Get the magic signature for the enter event.
    pub(crate) const fn magic_sig(&self) -> MagicSig {
        MagicSig {
            ty: MagicSigInfo::Enter,
            txid: self.enter.tx_hash(),
            event_idx: self.enter.log_index,
        }
    }

    /// Get the reth transaction signature for the enter event.
    pub(crate) fn signature(&self) -> Signature {
        self.magic_sig().into()
    }
}

impl ToRethPrimitive for Enter<'_, '_> {
    type RethPrimitive = TransactionSigned;

    fn to_reth(self) -> Self::RethPrimitive {
        TransactionSigned::new_unhashed(
            Transaction::Eip1559(TxEip1559 {
                chain_id: self.enter.rollup_chain_id(),
                nonce: self.nonce,
                gas_limit: BASE_TX_GAS_COST,
                max_fee_per_gas: 0,
                max_priority_fee_per_gas: 0,
                to: self.enter.rollupRecipient.into(),
                value: self.enter.amount,
                access_list: Default::default(),
                input: Default::default(),
            }),
            self.signature(),
        )
    }
}

/// Contains information necessary to produce a [`TransactionSigned`] for the
/// extracted [`Passage::EnterToken`] event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EnterToken<'a, 'b> {
    /// The extracted event for the enter token event.
    pub enter_token: &'a ExtractedEvent<'b, Passage::EnterToken>,
    /// The nonce of the transaction.
    pub nonce: u64,
    /// The address of the token being minted.
    pub token: Address,
}

impl EnterToken<'_, '_> {
    /// Get the magic signature for the enter token event.
    pub(crate) const fn magic_sig(&self) -> MagicSig {
        MagicSig {
            ty: MagicSigInfo::EnterToken,
            txid: self.enter_token.tx_hash(),
            event_idx: self.enter_token.log_index,
        }
    }

    /// Get the reth transaction signature for the enter token event.
    pub(crate) fn signature(&self) -> Signature {
        self.magic_sig().into()
    }
}
impl Tx for EnterToken<'_, '_> {
    fn fill_tx_env(&self, tx_env: &mut TxEnv) {
        self.enter_token.fill_tx_env(tx_env);
        tx_env.transact_to = TransactTo::Call(self.token);
        tx_env.nonce = Some(self.nonce);
    }
}

impl ToRethPrimitive for EnterToken<'_, '_> {
    type RethPrimitive = TransactionSigned;

    fn to_reth(self) -> Self::RethPrimitive {
        let input = zenith_types::mintCall {
            amount: self.enter_token.amount(),
            to: self.enter_token.rollupRecipient,
        }
        .abi_encode()
        .into();

        TransactionSigned::new_unhashed(
            Transaction::Eip1559(TxEip1559 {
                chain_id: self.enter_token.rollup_chain_id(),
                nonce: self.nonce,
                gas_limit: BASE_TX_GAS_COST,
                max_fee_per_gas: 0,
                max_priority_fee_per_gas: 0,
                // NB: set to the address of the token contract.
                to: self.token.into(),
                value: U256::ZERO,
                access_list: Default::default(),
                input, // NB: set to the ABI-encoded input for the `mint` function, which dictates the amount and recipient.
            }),
            self.signature(),
        )
    }
}

// Reth does not preserve envelope status for receipts, so
// the DB model will not support envelopes.
impl ToRethPrimitive for ReceiptEnvelope {
    type RethPrimitive = reth::primitives::Receipt;

    fn to_reth(self) -> Self::RethPrimitive {
        let success = self.is_success();
        let cumulative_gas_used = self.cumulative_gas_used();
        let tx_type = match self.tx_type() {
            alloy::consensus::TxType::Legacy => reth::primitives::TxType::Legacy,
            alloy::consensus::TxType::Eip2930 => reth::primitives::TxType::Eip2930,
            alloy::consensus::TxType::Eip1559 => reth::primitives::TxType::Eip1559,
            alloy::consensus::TxType::Eip4844 => reth::primitives::TxType::Eip4844,
            alloy::consensus::TxType::Eip7702 => reth::primitives::TxType::Eip7702,
        };

        let r = match self {
            ReceiptEnvelope::Legacy(r)
            | ReceiptEnvelope::Eip2930(r)
            | ReceiptEnvelope::Eip1559(r)
            | ReceiptEnvelope::Eip4844(r) => r,
            _ => panic!("unsupported receipt type"),
        };

        reth::primitives::Receipt { tx_type, success, cumulative_gas_used, logs: r.receipt.logs }
    }
}
