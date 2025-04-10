//! Signet RPC receipt response builder.

use alloy::consensus::Transaction;
use alloy::consensus::{transaction::TransactionMeta, ReceiptEnvelope, TxReceipt};
use alloy::primitives::{Address, TxKind};
use alloy::rpc::types::eth::{Log, ReceiptWithBloom, TransactionReceipt};
use reth::primitives::transaction::SignedTransaction;
use reth::primitives::{Receipt, TransactionSigned, TxType};
use reth::rpc::server_types::eth::{EthApiError, EthResult};
use signet_types::MagicSig;

/// Builds an [`TransactionReceipt`] obtaining the inner receipt envelope from the given closure.
pub fn build_signet_receipt(
    transaction: &TransactionSigned,
    meta: TransactionMeta,
    receipt: &Receipt,
    all_receipts: &[Receipt],
) -> EthResult<TransactionReceipt<ReceiptEnvelope<reth::rpc::types::Log>>>
where
{
    // Recover the transaction sender.
    // Some transactions are emitted by Signet itself in behalf of the sender,
    // in which case they'll use [`MagicSig`]s to preserve the sender with additional metadata.
    // Therefore, in case recovering the signer fails, we try to parse the signature as a magic signature.
    let from = transaction.recover_signer_unchecked().or_else(|_| {
        MagicSig::try_from_signature(transaction.signature())
            .map(|magic_sig| magic_sig.sender())
            .ok_or(EthApiError::InvalidTransactionSignature)
    })?;

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

    // Retrieve all corresponding logs for the receipt.
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
        inner: build_envelope(
            ReceiptWithBloom { receipt: rpc_receipt, logs_bloom },
            transaction.tx_type(),
        ),
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

fn build_envelope(
    receipt_with_bloom: ReceiptWithBloom<alloy::consensus::Receipt<reth::rpc::types::Log>>,
    tx_type: TxType,
) -> ReceiptEnvelope<reth::rpc::types::Log> {
    match tx_type {
        TxType::Legacy => ReceiptEnvelope::Legacy(receipt_with_bloom),
        TxType::Eip2930 => ReceiptEnvelope::Eip2930(receipt_with_bloom),
        TxType::Eip1559 => ReceiptEnvelope::Eip1559(receipt_with_bloom),
        TxType::Eip4844 => ReceiptEnvelope::Eip4844(receipt_with_bloom),
        TxType::Eip7702 => ReceiptEnvelope::Eip7702(receipt_with_bloom),
        #[allow(unreachable_patterns)]
        _ => unreachable!(),
    }
}

// Some code in this file has been copied and modified from reth
// <https://github.com/paradigmxyz/reth>
// The original license is included below:
//
// The MIT License (MIT)
//
// Copyright (c) 2022-2025 Reth Contributors
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//.
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.
