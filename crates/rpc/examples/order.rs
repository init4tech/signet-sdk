use alloy::{
    primitives::{Address, U256},
    signers::Signer,
};
use chrono::Utc;
use eyre::Error;
use signet_rpc::TxCache;
use signet_types::UnsignedOrder;
use signet_zenith::RollupOrders::{Input, Order, Output};

/// Helper fn to convert from a human readable amount to a U256 token amount.
fn token_amount(amount: u64, decimals: u32) -> U256 {
    U256::from(amount * 10u64.pow(decimals))
}

/// Empty main to silence clippy.
fn main() {}

/// Example code demonstrating API usage and patterns for signing an Order.
#[derive(Debug)]
pub struct SendOrder<S: Signer> {
    /// The signer to use for signing the order.
    signer: S,
    /// The transaction cache endpoint.
    tx_cache: TxCache,
    /// The address of the Order contract on the rollup.
    ru_order_contract: Address,
    /// The address of USDC on the rollup.
    ru_usdc_address: Address,
    /// The address of USDC on the host.
    host_usdc_address: Address,
    /// The chain id of the rollup.
    ru_chain_id: u64,
    /// The chain id of the host.
    host_chain_id: u64,
}

impl<S> SendOrder<S>
where
    S: Signer,
{
    /// Create a new SendOrder instance.
    pub const fn new(
        signer: S,
        tx_cache: TxCache,
        ru_order_contract: Address,
        ru_usdc_address: Address,
        host_usdc_address: Address,
        ru_chain_id: u64,
        host_chain_id: u64,
    ) -> Self {
        Self {
            signer,
            tx_cache,
            ru_order_contract,
            ru_usdc_address,
            host_usdc_address,
            ru_chain_id,
            host_chain_id,
        }
    }

    /// Construct a simple example Order, sign it, and send it.
    pub async fn run(&self) -> Result<(), Error> {
        // get an example order
        let order = self.example_order();

        // sign and send the order
        self.sign_and_send_order(order).await
    }

    /// Sign an Order and send it to the transaction cache to be Filled.
    pub async fn sign_and_send_order(&self, order: Order) -> Result<(), Error> {
        // make an UnsignedOrder from the Order
        let unsigned = UnsignedOrder::from(&order);

        // sign it
        let signed = unsigned
            .with_chain(self.ru_chain_id, self.ru_order_contract)
            .sign(&self.signer)
            .await?;

        // send the SignedOrder to the transaction cache
        self.tx_cache.forward_order(signed).await
    }

    /// Get an example Order which swaps 1 USDC on the rollup for 1 USDC on the host.
    fn example_order(&self) -> Order {
        let usdc_decimals: u32 = 6;
        let one_usdc = token_amount(1, usdc_decimals);

        // input is 1 USDC on the rollup
        let input = Input { token: self.ru_usdc_address, amount: one_usdc };

        // output is 1 USDC on the host chain
        let output = Output {
            token: self.host_usdc_address,
            amount: one_usdc,
            chainId: self.host_chain_id as u32,
            recipient: self.signer.address(),
        };

        // deadline 60 seconds (or ~5 blocks) from now
        let deadline = Utc::now().timestamp() + 60;

        // construct the order
        Order { inputs: vec![input], outputs: vec![output], deadline: U256::from(deadline) }
    }
}
