use alloy::{
    eips::Encodable2718,
    network::{Ethereum, EthereumWallet, TransactionBuilder},
    primitives::{Address, Bytes},
    providers::{
        fillers::{
            BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller,
            WalletFiller,
        },
        Identity, Provider as _, RootProvider, SendableTx,
    },
    rpc::types::{mev::EthSendBundle, TransactionRequest},
    signers::Signer,
};
use eyre::{eyre, Error};
use signet_bundle::SignetEthBundle;
use signet_rpc::TxCache;
use signet_types::{AggregateOrders, SignedFill, SignedOrder, UnsignedFill};
use std::collections::HashMap;

/// Multiplier for converting gwei to wei.
const GWEI_TO_WEI: u64 = 1_000_000_000;

/// Type alias for the provider used to build and submit transactions to the rollup and host.
type Provider = FillProvider<
    JoinFill<
        JoinFill<
            Identity,
            JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
        >,
        WalletFiller<EthereumWallet>,
    >,
    RootProvider,
    Ethereum,
>;

/// Example code demonstrating API usage and patterns for Signet Fillers.
#[derive(Debug)]
pub struct Filler<S: Signer> {
    /// The signer to use for signing transactions.
    signer: S,
    /// The provider to use for building transactions on the Rollup.
    ru_provider: Provider,
    /// The transaction cache endpoint.
    tx_cache: TxCache,
    /// A HashMap of the Order contract addresses for each chain.
    /// MUST contain an address for both Host and Rollup.
    order_contracts: HashMap<u64, Address>,
    /// The chain id of the rollup.
    ru_chain_id: u64,
    /// The chain id of the host.
    host_chain_id: u64,
}

impl<S> Filler<S>
where
    S: Signer,
{
    /// Create a new Filler with the given signer, provider, and transaction cache endpoint.
    pub const fn new(
        signer: S,
        ru_provider: Provider,
        tx_cache: TxCache,
        order_contracts: HashMap<u64, Address>,
        ru_chain_id: u64,
        host_chain_id: u64,
    ) -> Self {
        Self { signer, ru_provider, tx_cache, order_contracts, ru_chain_id, host_chain_id }
    }

    /// Fills Orders by aggregating them into a single, atomic Bundle.
    ///
    /// Filling orders in aggregate means that Fills are batched and more gas efficient;
    /// however, if a single Order cannot be filled, then the entire Bundle will not mine.
    ///
    /// For example, using this strategy, if one Order is filled by another Filler first, then all other Orders will also not be filled.
    ///
    /// It may be a preferred strategy to fill each Order in a separate Bundle, as in `fill_individual`.
    pub async fn fill_aggregate(&self) -> Result<(), Error> {
        let fillable_orders = self.get_fillable_orders().await?;

        // submit one bundle that fills the entire set of orders
        self.fill(fillable_orders).await
    }

    /// Fills Orders individually, by submitting a separate Bundle for each Order.
    ///
    /// Filling Orders individually ensures that even if some Orders are not fillable, others may still mine;
    /// however, it is less gas efficient.
    ///
    /// It may be a preferred strategy to fill Orders within a single Bundle, as in `fill_aggregate`.
    pub async fn fill_individual(&self) -> Result<(), Error> {
        let fillable_orders = self.get_fillable_orders().await?;

        // submit one bundle per individual order
        for order in fillable_orders {
            self.fill(vec![order]).await?;
        }

        Ok(())
    }

    /// Query the transaction cache to get all possible orders.
    pub async fn get_orders(&self) -> Result<Vec<SignedOrder>, Error> {
        self.tx_cache.get_orders().await
    }

    /// Query the transaction cache to get all possible orders and filter them down based on the provided logic.
    ///
    /// This is a simple, naive way of filtering the orders down to those to attempt to fill.
    /// Fillers may implement more complex business logic that creates bespoke groupings of Orders.
    pub async fn get_fillable_orders(&self) -> Result<Vec<SignedOrder>, Error> {
        let all_orders = self.get_orders().await?;

        // filter the SignedOrders based on the provided function
        self.filter_orders(all_orders).await
    }

    /// Fillers should implement bespoke business logic to filter orders
    /// down to those they are capable of filling & desire to fill.
    async fn filter_orders(&self, _orders: Vec<SignedOrder>) -> Result<Vec<SignedOrder>, Error> {
        todo!()
    }

    /// Construct a Bundle to fill the selected set of orders.
    pub async fn fill(&self, orders: Vec<SignedOrder>) -> Result<(), Error> {
        // if orders is empty, exit the function without doing anything
        if orders.is_empty() {
            println!("No orders to fill");
            return Ok(());
        }

        // sign a SignedFill for the orders
        let signed_fills = self.sign_fills(orders.clone()).await?;

        // get the transaction requests for the rollup
        let tx_requests = self.rollup_txn_requests(&signed_fills, &orders).await?;

        // sign & encode the transactions for the Bundle
        let txs = self.sign_and_encode_txns(tx_requests).await?;

        // get the aggregated host fill for the Bundle, if any
        let host_fills = signed_fills.get(&self.host_chain_id).cloned();

        // set the Bundle to only be valid if mined in the next rollup block
        let block_number = self.ru_provider.get_block_number().await? + 1;

        // construct a Bundle containing the Rollup transactions and the Host fill (if any)
        let bundle = SignetEthBundle {
            host_fills,
            bundle: EthSendBundle {
                txs,
                reverting_tx_hashes: vec![], // generally, if the Order initiations revert, then fills should not be submitted
                block_number,
                min_timestamp: None, // sufficiently covered by pinning to next block number
                max_timestamp: None, // sufficiently covered by pinning to next block number
                replacement_uuid: None, // optional if implementing strategies that replace or cancel bundles
            },
        };

        // submit the Bundle to the transaction cache
        self.tx_cache.forward_bundle(bundle).await
    }

    /// Aggregate the given orders into a SignedFill, sign it, and
    /// return a HashMap of SignedFills for each destination chain.
    ///
    /// This is the simplest, minimally viable way to turn a set of SignedOrders into a single Aggregated Fill on each chain;
    /// Fillers may wish to implement more complex setups.
    ///
    /// For example, if utilizing different signers for each chain, they may use `UnsignedFill.sign_for(chain_id)` instead of `sign()`.
    ///
    /// If filling multiple Orders, they may wish to utilize one Order's Outputs to provide another Order's rollup Inputs.
    /// In this case, the Filler would wish to split up the Fills for each Order,
    /// rather than signing a single, aggregate a Fill for each chain, as is done here.
    pub async fn sign_fills(
        &self,
        orders: Vec<SignedOrder>,
    ) -> Result<HashMap<u64, SignedFill>, Error> {
        //  create an AggregateOrder from the SignedOrders they want to fill
        let agg = AggregateOrders::from(orders);
        // produce an UnsignedFill from the AggregateOrder
        let mut unsigned_fill = UnsignedFill::from(&agg);
        // populate the Order contract addresses for each chain
        for chain_id in agg.destination_chain_ids() {
            unsigned_fill =
                unsigned_fill.with_chain(chain_id, self.order_contract_address_for(chain_id)?);
        }
        // sign the UnsignedFill, producing a SignedFill for each target chain
        Ok(unsigned_fill.sign(&self.signer).await?)
    }

    /// Construct a set of transaction requests to be submitted on the rollup.
    ///
    /// Perform a single, aggregate Fill upfront, then Initiate each Order.
    /// Transaction requests look like [`fill_aggregate`, `initiate_1`, `initiate_2`].
    ///
    /// This is the simplest, minimally viable way to get a set of Orders mined;
    /// Fillers may wish to implement more complex strategies.
    ///
    /// For example, Fillers might utilize one Order's Inputs to fill subsequent Orders' Outputs.
    /// In this case, the rollup transactions should look like [`fill_1`, `inititate_1`, `fill_2`, `initiate_2`].
    async fn rollup_txn_requests(
        &self,
        signed_fills: &HashMap<u64, SignedFill>,
        orders: &Vec<SignedOrder>,
    ) -> Result<Vec<TransactionRequest>, Error> {
        // construct the transactions to be submitted to the Rollup
        let mut tx_requests = Vec::new();

        // first, if there is a SignedFill for the Rollup, add a transaction to submit the fill
        // Note that `fill` transactions MUST be mined *before* the corresponding Order(s) `initiate` transactions in order to cound
        // Host `fill` transactions are always considered to be mined "before" the rollup block is processed,
        // but Rollup `fill` transactions MUST take care to be ordered before the Orders are `initiate`d
        if let Some(rollup_fill) = signed_fills.get(&self.ru_chain_id) {
            // add the fill tx to the rollup txns
            let ru_fill_tx = rollup_fill.to_fill_tx(self.ru_order_contract()?);
            tx_requests.push(ru_fill_tx);
        }

        // next, add a transaction to initiate each SignedOrder
        for signed_order in orders {
            // add the initiate tx to the rollup txns
            let ru_initiate_tx =
                signed_order.to_initiate_tx(self.signer.address(), self.ru_order_contract()?);
            tx_requests.push(ru_initiate_tx);
        }

        Ok(tx_requests)
    }

    /// Given an ordered set of Transaction Requests,
    /// Sign them and encode them for inclusion in a Bundle.
    pub async fn sign_and_encode_txns(
        &self,
        tx_requests: Vec<TransactionRequest>,
    ) -> Result<Vec<Bytes>, Error> {
        let mut encoded_txs: Vec<Bytes> = Vec::new();
        for mut tx in tx_requests {
            // fill out the transaction fields
            tx = tx
                .with_from(self.signer.address())
                .with_gas_limit(1_000_000)
                .with_max_priority_fee_per_gas((GWEI_TO_WEI * 16) as u128);

            // sign the transaction
            let SendableTx::Envelope(filled) = self.ru_provider.fill(tx).await? else {
                return Err(eyre!("Failed to fill rollup transaction"));
            };

            // encode it
            let encoded = filled.encoded_2718();

            // add to array
            encoded_txs.push(Bytes::from(encoded));
        }
        Ok(encoded_txs)
    }

    /// Get the Order contract address for the given chain id.
    fn order_contract_address_for(&self, chain_id: u64) -> Result<Address, Error> {
        self.order_contracts
            .get(&chain_id)
            .cloned()
            .ok_or(eyre!("No Order contract address configured for chain id {}", chain_id))
    }

    /// Get the Order contract address for the rollup.
    fn ru_order_contract(&self) -> Result<Address, Error> {
        self.order_contract_address_for(self.ru_chain_id)
    }
}
