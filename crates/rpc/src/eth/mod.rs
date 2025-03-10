mod endpoints;
use endpoints::*;

mod error;
pub use error::EthError;

mod forwarder;
pub use forwarder::TxCacheForwarder;

mod helpers;
pub use helpers::CallErrorData;

use crate::{ctx::RpcCtx, Pnt};
use alloy::{eips::BlockNumberOrTag, primitives::B256};
use reth_node_api::FullNodeComponents;

/// Instantiate the `eth` API router.
pub fn eth<Host, Signet>() -> ajj::Router<RpcCtx<Host, Signet>>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    ajj::Router::new()
        .route("protocolVersion", protocol_version)
        .route("syncing", syncing)
        .route("blockNumber", block_number)
        .route("chainId", chain_id)
        .route("getBlockByHash", block::<B256, _, _>)
        .route("getBlockByNumber", block::<BlockNumberOrTag, _, _>)
        .route("getBlockTransactionCountByHash", block_tx_count::<B256, _, _>)
        .route("getBlockTransactionCountByNumber", block_tx_count::<BlockNumberOrTag, _, _>)
        .route("getBlockReceipts", block_receipts)
        .route("getRawTransactionByHash", raw_transaction_by_hash)
        .route("getTransactionByHash", transaction_by_hash)
        .route(
            "getRawTransactionByBlockHashAndIndex",
            raw_transaction_by_block_and_index::<B256, _, _>,
        )
        .route(
            "getRawTransactionByBlockNumberAndIndex",
            raw_transaction_by_block_and_index::<BlockNumberOrTag, _, _>,
        )
        .route("getTransactionByBlockHashAndIndex", transaction_by_block_and_index::<B256, _, _>)
        .route(
            "getTransactionByBlockNumberAndIndex",
            transaction_by_block_and_index::<BlockNumberOrTag, _, _>,
        )
        .route("getTransactionReceipt", transaction_receipt)
        .route("getBalance", balance)
        .route("getStorageAt", storage_at)
        .route("getTransactionCount", addr_tx_count)
        .route("getCode", code_at)
        .route("getBlockHeaderByHash", header_by::<B256, _, _>)
        .route("getBlockHeaderByNumber", header_by::<BlockNumberOrTag, _, _>)
        .route("call", call)
        .route("estimateGas", estimate_gas)
        .route("gasPrice", gas_price)
        .route("maxPriorityFeePerGas", max_priority_fee_per_gas)
        .route("feeHistory", fee_history)
        .route("sendRawTransaction", send_raw_transaction)
        .route("getLogs", get_logs)
        .route("newFilter", new_filter)
        .route("newBlockFilter", new_block_filter)
        .route("uninstallFilter", uninstall_filter)
        .route("getFilterChanges", get_filter_changes)
        .route("getFilterLogs", get_filter_changes)
        .route("subscribe", subscribe)
        .route("unsubscribe", unsubscribe)
        // ---------------
        //
        // Unsupported methods:
        //
        .route("coinbase", not_supported)
        .route("accounts", not_supported)
        .route("blobBaseFee", not_supported)
        .route("getUncleCountByBlockHash", not_supported)
        .route("getUncleCountByBlockNumber", not_supported)
        .route("getUncleByBlockHashAndIndex", not_supported)
        .route("getUncleByBlockNumberAndIndex", not_supported)
        .route("getWork", not_supported)
        .route("hashrate", not_supported)
        .route("mining", not_supported)
        .route("submitHashrate", not_supported)
        .route("submitWork", not_supported)
        .route("sendTransaction", not_supported)
        .route("sign", not_supported)
        .route("signTransaction", not_supported)
        .route("signTypedData", not_supported)
        .route("getProof", not_supported)
        .route("createAccessList", not_supported)
        .route("newPendingTransactionFilter", not_supported)
}
