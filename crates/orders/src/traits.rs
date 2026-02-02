#[cfg(doc)]
use crate::Filler;
use crate::OrdersAndFills;
use alloy::{
    network::{Ethereum, Network},
    providers::{fillers::FillerControlFlow, Provider, SendableTx},
    transports::TransportResult,
};
use core::future::Future;
use futures_util::Stream;
use signet_bundle::SignetEthBundle;
use signet_types::SignedOrder;

/// A trait for submitting signed orders to a backend.
///
/// Implementors of this trait are responsible for forwarding signed orders to a transaction cache
/// or other order submission endpoint.
pub trait OrderSubmitter {
    /// The error type returned by submission operations.
    type Error: core::error::Error + Send + Sync + 'static;

    /// Submit a signed order to the backend.
    fn submit_order(
        &self,
        order: SignedOrder,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

/// A trait for fetching orders from a source.
///
/// Implementors of this trait provide access to signed orders, typically from a transaction cache.
pub trait OrderSource {
    /// The error type returned by the stream.
    type Error: core::error::Error + Send + Sync + 'static;

    /// Fetch orders from the source as a stream.
    ///
    /// Returns a stream of orders that automatically handles pagination. The stream yields
    /// `Result<SignedOrder, Self::Error>` to allow for error propagation during iteration.
    fn get_orders(&self) -> impl Stream<Item = Result<SignedOrder, Self::Error>> + Send;
}

/// A trait for submitting bundles to a backend.
///
/// Implementors of this trait are responsible for forwarding bundles to a transaction cache or
/// builder endpoint.
pub trait BundleSubmitter {
    /// The response type returned on successful submission.
    type Response;
    /// The error type returned by submission operations.
    type Error: core::error::Error + Send + Sync + 'static;

    /// Submit a bundle to the backend.
    fn submit_bundle(
        &self,
        bundle: SignetEthBundle,
    ) -> impl Future<Output = Result<Self::Response, Self::Error>> + Send;
}

/// A provider that can fill transactions.
///
/// This trait abstracts over [`FillProvider`] to allow filling transaction requests.
pub trait TxBuilder<N: Network = Ethereum>: Provider<N> + Send + Sync {
    /// Fill a transaction request, returning a sendable transaction.
    fn fill(
        &self,
        tx: N::TransactionRequest,
    ) -> impl Future<Output = TransportResult<SendableTx<N>>> + Send;

    /// Return the filler's status for the given transaction request.
    fn status(&self, tx: &N::TransactionRequest) -> FillerControlFlow;
}

/// A trait for submitting signed fills to a backend.
///
/// Implementors handle transaction construction, gas pricing, and target block determination.
/// This decouples the [`Filler`] from provider and fee concerns.
pub trait FillSubmitter {
    /// The response type returned on successful submission.
    type Response;
    /// The error type returned by submission operations.
    type Error: core::error::Error + Send + Sync + 'static;

    /// Submit signed fills to the backend.
    ///
    /// The fills map contains one [`SignedFill`] per destination chain ID.
    fn submit_fills(
        &self,
        orders_and_fills: OrdersAndFills,
    ) -> impl Future<Output = Result<Self::Response, Self::Error>> + Send;
}
