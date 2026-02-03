//! Mock implementations and test helpers for signet-orders traits.
use crate::users::TEST_SIGNERS;
use alloy::{
    network::{Ethereum, EthereumWallet, TransactionBuilder},
    primitives::{Address, U256},
    providers::{fillers::FillerControlFlow, Provider, ProviderBuilder, RootProvider, SendableTx},
    signers::{local::PrivateKeySigner, Signer},
    transports::{mock::Asserter, TransportResult},
};
use core::convert::Infallible;
use futures_util::{stream, Stream};
use signet_bundle::SignetEthBundle;
use signet_constants::{test_utils::TEST_SYS, SignetSystemConstants};
use signet_orders::{
    BundleSubmitter, FillSubmitter, OrderSource, OrderSubmitter, OrdersAndFills, TxBuilder,
};
use signet_types::{SignedOrder, UnsignedOrder};
use signet_zenith::RollupOrders::Output;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

/// A mock [`OrderSubmitter`] that captures submitted orders.
#[derive(Debug, Clone, Default)]
pub struct MockOrderSubmitter {
    orders: Arc<Mutex<Vec<SignedOrder>>>,
}

impl MockOrderSubmitter {
    /// Create a new mock order submitter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all submitted orders.
    pub fn submitted_orders(&self) -> Vec<SignedOrder> {
        self.orders.lock().unwrap().clone()
    }
}

impl OrderSubmitter for MockOrderSubmitter {
    type Error = Infallible;

    async fn submit_order(&self, order: SignedOrder) -> Result<(), Self::Error> {
        self.orders.lock().unwrap().push(order);
        Ok(())
    }
}

/// A mock [`OrderSource`] that returns a predefined list of orders.
#[derive(Debug, Clone)]
pub struct MockOrderSource {
    orders: Vec<SignedOrder>,
}

impl MockOrderSource {
    /// Create a new mock order source with the given orders.
    pub fn new(orders: Vec<SignedOrder>) -> Self {
        Self { orders }
    }

    /// Create an empty mock order source.
    pub fn empty() -> Self {
        Self { orders: vec![] }
    }
}

impl OrderSource for MockOrderSource {
    type Error = Infallible;

    fn get_orders(&self) -> impl Stream<Item = Result<SignedOrder, Self::Error>> + Send {
        stream::iter(self.orders.clone().into_iter().map(Ok))
    }
}

/// A mock [`BundleSubmitter`] that captures submitted bundles.
#[derive(Debug, Clone, Default)]
pub struct MockBundleSubmitter {
    bundles: Arc<Mutex<Vec<SignetEthBundle>>>,
}

impl MockBundleSubmitter {
    /// Create a new mock bundle submitter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all submitted bundles.
    pub fn submitted_bundles(&self) -> Vec<SignetEthBundle> {
        self.bundles.lock().unwrap().clone()
    }
}

impl BundleSubmitter for MockBundleSubmitter {
    type Response = ();
    type Error = Infallible;

    async fn submit_bundle(&self, bundle: SignetEthBundle) -> Result<(), Self::Error> {
        self.bundles.lock().unwrap().push(bundle);
        Ok(())
    }
}

/// A mock [`TxBuilder`] that pre-fills transactions with gas and nonce values.
///
/// This avoids the complexity of concurrent filler RPC calls by setting values locally:
/// - Nonce: incremented locally for each transaction
/// - Gas limit: fixed at 100,000
/// - Max fee per gas: fixed at 1 gwei
/// - Max priority fee per gas: fixed at 1 gwei
///
/// The inner provider is only used for signing and `get_block_number` calls.
#[derive(Clone)]
pub struct MockTxBuilder<P> {
    inner: P,
    asserter: Asserter,
    nonce: Arc<AtomicU64>,
}

impl<P> MockTxBuilder<P> {
    /// Create a new mock transaction builder wrapping the given provider.
    fn new(inner: P, asserter: Asserter) -> Self {
        Self { inner, asserter, nonce: Arc::new(AtomicU64::new(0)) }
    }

    /// Get a reference to the asserter for pushing mock responses.
    pub fn asserter(&self) -> &Asserter {
        &self.asserter
    }
}

impl<P: Provider<Ethereum>> Provider<Ethereum> for MockTxBuilder<P> {
    fn root(&self) -> &RootProvider<Ethereum> {
        self.inner.root()
    }
}

impl<P> TxBuilder<Ethereum> for MockTxBuilder<P>
where
    P: TxBuilder<Ethereum>,
{
    async fn fill(
        &self,
        mut tx: <Ethereum as alloy::network::Network>::TransactionRequest,
    ) -> TransportResult<SendableTx<Ethereum>> {
        // Pre-fill gas and nonce if they're not already set, so fillers don't need to make RPC
        // calls.
        if tx.nonce.is_none() {
            let nonce = self.nonce.fetch_add(1, Ordering::SeqCst);
            tx = tx.with_nonce(nonce);
        }
        if tx.gas.is_none() {
            tx = tx.with_gas_limit(100_000);
        }
        if tx.max_fee_per_gas.is_none() {
            tx = tx.with_max_fee_per_gas(1_000_000_000); // 1 gwei
        }
        if tx.max_priority_fee_per_gas.is_none() {
            tx = tx.with_max_priority_fee_per_gas(1_000_000_000); // 1 gwei
        }
        self.inner.fill(tx).await
    }

    fn status(
        &self,
        tx: &<Ethereum as alloy::network::Network>::TransactionRequest,
    ) -> FillerControlFlow {
        self.inner.status(tx)
    }
}

/// Create a mock [`TxBuilder`] for testing transaction building without a real network.
///
/// Pre-fills transactions with gas and nonce values locally, so the only RPC call needed
/// is `get_block_number`.
pub fn mock_tx_builder(
    wallet: PrivateKeySigner,
    chain_id: u64,
) -> MockTxBuilder<impl TxBuilder<Ethereum>> {
    let asserter = Asserter::new();
    let inner = ProviderBuilder::new()
        .with_chain_id(chain_id)
        .wallet(EthereumWallet::new(wallet))
        .connect_mocked_client(asserter.clone());
    MockTxBuilder::new(inner, asserter)
}

/// A mock [`FillSubmitter`] that captures submitted fills.
#[derive(Debug, Clone, Default)]
pub struct MockFillSubmitter {
    submissions: Arc<Mutex<Vec<OrdersAndFills>>>,
}

impl MockFillSubmitter {
    /// Create a new mock fill submitter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all submitted fills.
    pub fn submissions(&self) -> Vec<OrdersAndFills> {
        self.submissions.lock().unwrap().clone()
    }
}

impl FillSubmitter for MockFillSubmitter {
    type Response = ();
    type Error = Infallible;

    async fn submit_fills(&self, orders_and_fills: OrdersAndFills) -> Result<(), Self::Error> {
        self.submissions.lock().unwrap().push(orders_and_fills);
        Ok(())
    }
}

/// Builder for creating test [`SignedOrder`] instances.
#[derive(Debug, Clone)]
pub struct TestOrderBuilder {
    constants: SignetSystemConstants,
    inputs: Vec<(Address, U256)>,
    outputs: Vec<Output>,
    nonce: Option<u64>,
}

impl Default for TestOrderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TestOrderBuilder {
    /// Create a new test order builder using [`TEST_SYS`] system constants.
    pub fn new() -> Self {
        Self { constants: TEST_SYS, inputs: vec![], outputs: vec![], nonce: None }
    }

    /// Use the provided system constants.
    pub fn with_constants(mut self, constants: SignetSystemConstants) -> Self {
        self.constants = constants;
        self
    }

    /// Append a new input to the collection of inputs.
    pub fn with_input(mut self, token: Address, amount: U256) -> Self {
        self.inputs.push((token, amount));
        self
    }

    /// Append a new output to the collection of outputs.
    pub fn with_output(
        mut self,
        token: Address,
        amount: U256,
        recipient: Address,
        chain_id: u64,
    ) -> Self {
        self.outputs.push(Output { token, amount, recipient, chainId: chain_id as u32 });
        self
    }

    /// Set the nonce.
    pub fn with_nonce(mut self, nonce: u64) -> Self {
        self.nonce = Some(nonce);
        self
    }

    /// Sign and build the order.
    pub async fn sign<S: Signer>(self, signer: &S) -> SignedOrder {
        let mut unsigned = UnsignedOrder::new();

        for (token, amount) in self.inputs {
            unsigned = unsigned.with_input(token, amount);
        }

        for output in self.outputs {
            unsigned = unsigned.with_raw_output(output);
        }

        if let Some(nonce) = self.nonce {
            unsigned = unsigned.with_nonce(nonce);
        }

        unsigned = unsigned.with_chain(&self.constants);

        unsigned.sign(signer).await.expect("signing should succeed with test signer")
    }
}

/// Create dummy orders for testing: one with host chain output, one with rollup chain output.
pub async fn default_test_orders() -> Vec<SignedOrder> {
    let signer = &TEST_SIGNERS[0];

    let host_order = TestOrderBuilder::new()
        .with_input(Address::repeat_byte(0x11), U256::from(1000))
        .with_output(
            Address::repeat_byte(0x22),
            U256::from(500),
            signer.address(),
            TEST_SYS.host_chain_id(),
        )
        .with_nonce(1)
        .sign(signer)
        .await;

    let rollup_order = TestOrderBuilder::new()
        .with_input(Address::repeat_byte(0x11), U256::from(2000))
        .with_output(
            Address::repeat_byte(0x33),
            U256::from(1000),
            signer.address(),
            TEST_SYS.ru_chain_id(),
        )
        .with_nonce(2)
        .sign(signer)
        .await;

    vec![host_order, rollup_order]
}
