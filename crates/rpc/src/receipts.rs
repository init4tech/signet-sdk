//! Signet RPC receipt response builder.

use alloy::consensus::{transaction::TransactionMeta, ReceiptEnvelope, TxReceipt};
use alloy::primitives::{Address, TxKind};
use alloy::rpc::types::eth::{Log, ReceiptWithBloom, TransactionReceipt};
use reth::primitives::transaction::SignedTransaction;
use reth::primitives::{Receipt, TransactionSigned, TxType};
use reth::rpc::server_types::eth::{EthApiError, EthResult};
use signet_types::MagicSig;

/// Builds an [`TransactionReceipt`] obtaining the inner receipt envelope from the given closure.
pub fn build_receipt<R, T, E>(
    transaction: &T,
    meta: TransactionMeta,
    receipt: &R,
    all_receipts: &[R],
    build_envelope: impl FnOnce(ReceiptWithBloom<alloy::consensus::Receipt<Log>>) -> E,
) -> EthResult<TransactionReceipt<E>>
where
    R: TxReceipt<Log = alloy::primitives::Log>,
    T: SignedTransaction,
{
    // Recover the transaction sender.
    // Some transactions are emitted by Signet itself in behalf of the sender,
    // in which case they'll use [`MagicSig`]s to preserve the sender with additional metadata.
    // Therefore, in case recovering the signer fails, we try to parse the signature as a magic signature.
    let from = match transaction.recover_signer_unchecked() {
        Ok(address) => address,
        Err(_) => {
            // If the transaction is not signed by the sender, it is a magic signature.
            let magic_sig = MagicSig::try_from_signature(transaction.signature())
                .ok_or_else(|| EthApiError::InvalidTransactionSignature)?;
            magic_sig.sender()
        }
    };

    // get the previous transaction cumulative gas used
    let gas_used = if meta.index == 0 {
        receipt.cumulative_gas_used()
    } else {
        let prev_tx_idx = (meta.index - 1) as usize;
        all_receipts
            .get(prev_tx_idx)
            .map(|prev_receipt| receipt.cumulative_gas_used() - prev_receipt.cumulative_gas_used())
            .unwrap_or_default()
    };

    let logs_bloom = receipt.bloom();

    // get number of logs in the block
    let mut num_logs = 0;
    for prev_receipt in all_receipts.iter().take(meta.index as usize) {
        num_logs += prev_receipt.logs().len();
    }

    let logs: Vec<Log> = receipt
        .logs()
        .iter()
        .enumerate()
        .map(|(tx_log_idx, log)| Log {
            inner: log.clone(),
            block_hash: Some(meta.block_hash),
            block_number: Some(meta.block_number),
            block_timestamp: Some(meta.timestamp),
            transaction_hash: Some(meta.tx_hash),
            transaction_index: Some(meta.index),
            log_index: Some((num_logs + tx_log_idx) as u64),
            removed: false,
        })
        .collect();

    let rpc_receipt = alloy::rpc::types::eth::Receipt {
        status: receipt.status_or_post_state(),
        cumulative_gas_used: receipt.cumulative_gas_used(),
        logs,
    };

    let (contract_address, to) = match transaction.kind() {
        TxKind::Create => (Some(from.create(transaction.nonce())), None),
        TxKind::Call(addr) => (None, Some(Address(*addr))),
    };

    Ok(TransactionReceipt {
        inner: build_envelope(ReceiptWithBloom { receipt: rpc_receipt, logs_bloom }),
        transaction_hash: meta.tx_hash,
        transaction_index: Some(meta.index),
        block_hash: Some(meta.block_hash),
        block_number: Some(meta.block_number),
        from,
        to,
        gas_used,
        contract_address,
        effective_gas_price: transaction.effective_gas_price(meta.base_fee),
        // Signet does not support EIP-4844, so these fields are always None.
        blob_gas_price: None,
        blob_gas_used: None,
    })
}

/// Receipt response builder.
#[derive(Debug)]
pub struct SignetReceiptBuilder {
    /// The base response body, contains L1 fields.
    pub base: TransactionReceipt,
}

impl SignetReceiptBuilder {
    /// Returns a new builder with the base response body (L1 fields) set.
    ///
    /// Note: This requires _all_ block receipts because we need to calculate the gas used by the
    /// transaction.
    pub fn new(
        transaction: &TransactionSigned,
        meta: TransactionMeta,
        receipt: &Receipt,
        all_receipts: &[Receipt],
    ) -> EthResult<Self> {
        let base = build_receipt(transaction, meta, receipt, all_receipts, |receipt_with_bloom| {
            match receipt.tx_type {
                TxType::Legacy => ReceiptEnvelope::Legacy(receipt_with_bloom),
                TxType::Eip2930 => ReceiptEnvelope::Eip2930(receipt_with_bloom),
                TxType::Eip1559 => ReceiptEnvelope::Eip1559(receipt_with_bloom),
                TxType::Eip4844 => ReceiptEnvelope::Eip4844(receipt_with_bloom),
                TxType::Eip7702 => ReceiptEnvelope::Eip7702(receipt_with_bloom),
                #[allow(unreachable_patterns)]
                _ => unreachable!(),
            }
        })?;

        Ok(Self { base })
    }

    /// Builds a receipt response from the base response body, and any set additional fields.
    pub fn build(self) -> TransactionReceipt {
        self.base
    }
}
