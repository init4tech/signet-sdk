use crate::rpc::cache::TxCache;
use crate::zenith::orders::{AggregateOrders, SignedOrder, UnsignedFill, SignedFill};
use crate::bundle::SignetEthBundle
use alloy::primitives::Address;
use alloy::signers::local::{LocalSignerError, PrivateKeySigner};

const TX_CACHE_ENDPOINT = "https://transactions.signet.sh";
const HOST_CHAIN_ID = 17000;
const RU_CHAIN_ID = 17001;

// implement configuration for each chain's Order contract address
const fn order_contract_address_for(chain_id: u6) -> Address {
    match chain_id {
        HOST_CHAIN_ID => todo!(),
        RU_CHAIN_ID => todo!(),
        _ => panic!("Unsupported chain ID"),
    }
}

/// Create a PrivateKeySigner from a hex-encoded private key.
fn wallet(private_key: &str) -> PrivateKeySigner {
    let bytes = hex::decode(private_key.strip_prefix("0x").unwrap_or(private_key));
    PrivateKeySigner::from_slice(&bytes).unwrap()
}

// implement business logic to filter the orders down to those you wish to fill
async fn filter_orders(orders: Vec<SignedOrder>) -> Result<Vec<SignedOrder>, eyre::Error> {
    todo!()
}

/// Implements basic logic to fill orders.
/// Queries the transaction cache to get all possible orders,
/// filters them down to those you wish to fill,
/// and constructs a Bundle to fill them in aggregate.
/// 
/// Note that filling orders in aggregate means that Fills are batched and more gas efficient; however,
/// if a single Order fails - for example, if it is filled by another Filler first - 
/// then the entire Bundle will not mine. 
/// It may be a preferred strategy to fill orders separately, constructing a Bundle for each SignedOrder.
async fn fill_aggregate() -> Result<(), eyre::Error> {
    // query the transaction cache to get an array of SignedOrders
    let tx_cache = TxCache::new(TX_CACHE_ENDPOINT);
    let all_orders = tx_cache.get_orders().await?;

    // filter the SignedOrders down to those you wish to fill
    let fillable_orders = filter_orders(all_orders).await?;

    // construct and submit a Bundle to fill the selected set of orders
    fill(fillable_orders).await
}

/// Construct a Bundle to fill the selected set of orders.
pub async fn fill(orders: Vec<SignedOrder>, signer: Signer) -> Result<(), eyre::Error> {
    if orders.is_empty() {
        println!("No orders to fill");
        return Ok(());
    }

    //  create an AggregateOrder from the SignedOrders they want to fill
let agg = AggregateOrders::from(orders);
// produce an UnsignedFill from the AggregateOrder
let mut unsigned_fill = UnsignedFill::from(agg);
// populate the Order contract addresses for each chain
for chain_id in agg.output_chain_ids() {
   unsigned_fill = unsigned_fill.with_chain(chain_id, order_contract_address_for(chain_id));
}

// sign the UnsignedFill, producing a SignedFill for each target chain
let signed_fills = unsigned_fill.sign(signer).await?;

// construct the transactions to be submitted to the Rollup
let mut rollup_txs = Vec::new();
// first, if there is a SignedFill for the Rollup, add a transaction to submit the fill
if Some(rollup_fill) = signed_fills.get(RU_CHAIN_ID) {
    // produce a Rollup transaction from the SignedFill
    let fill_tx = todo!();

    // add the Rollup fill to the rollup txns
    rollup_txs.push(fill_tx);
}

// next, add a transaction to submit each SignedOrder
orders.iter().for_each(|signed_order| {
    // produce a Rollup transaction from the SignedOrder
    let initiate_tx = todo!();

    // add the Rollup order to the rollup txns
    rollup_txs.push(initiate_tx);
});

// construct a Bundle containing the Rollup transactions and the Host fill (if any)
let bundle = SignetEthBundle {
   host_fills: signed_fills.get(HOST_CHAIN_ID),
   bundle: EthSendBundle {
    txs: rollup_txs,
    reverting_tx_hashes: vec![],
    min_timestamp: None,
    max_timestamp: None,
    block_number: 0,
    replacement_uuid: None,
   }
};

// submit the Bundle to the Bundle API
let tx_cache = TxCache::new(TX_CACHE_ENDPOINT);
tx_cache.forward_bundle(bundle);
}