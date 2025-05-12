use alloy::{
    primitives::{uint, U256},
    signers::Signer,
};
use chrono::Utc;
use eyre::Error;
use signet_constants::SignetConstants;
use signet_tx_cache::client::TxCache;
use signet_types::UnsignedOrder;
use signet_zenith::RollupOrders::{Input, Order, Output};

const ONE_USDC: U256 = uint!(1_000_000_U256);

/// Example code demonstrating API usage and patterns for signing an Order.
#[derive(Debug)]
pub struct SendOrder<S: Signer> {
    /// The signer to use for signing the order.
    signer: S,
    /// The transaction cache endpoint.
    tx_cache: TxCache,
    /// The system constants.
    constants: SignetConstants,
}

impl<S> SendOrder<S>
where
    S: Signer,
{
    /// Create a new SendOrder instance.
    pub fn new(signer: S, constants: SignetConstants) -> Result<Self, Error> {
        Ok(Self {
            signer,
            tx_cache: TxCache::from_environment(&constants.environment())?,
            constants,
        })
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
            .with_chain(self.constants.rollup().chain_id(), self.constants.rollup().orders())
            .sign(&self.signer)
            .await?;

        // send the SignedOrder to the transaction cache
        self.tx_cache.forward_order(signed).await
    }

    /// Get an example Order which swaps 1 USDC on the rollup for 1 USDC on the host.
    fn example_order(&self) -> Order {
        // input is 1 USDC on the rollup
        let input = Input { token: self.constants.rollup().tokens().usdc(), amount: ONE_USDC };

        // output is 1 USDC on the host chain
        let output = Output {
            token: self.constants.host().tokens().usdc(),
            amount: ONE_USDC,
            chainId: self.constants.host().chain_id() as u32,
            recipient: self.signer.address(),
        };

        // deadline 60 seconds (or ~5 blocks) from now
        let deadline = Utc::now().timestamp() + 60;

        // construct the order
        Order { inputs: vec![input], outputs: vec![output], deadline: U256::from(deadline) }
    }
}

/// Empty main to silence clippy.
fn main() {}
