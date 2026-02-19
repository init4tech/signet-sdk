use crate::{BundleSubmitter, OrderSource, OrderSubmitter};
use futures_util::stream::Stream;
use signet_bundle::SignetEthBundle;
use signet_tx_cache::{types::BundleResponse, TxCache, TxCacheError};
use signet_types::SignedOrder;

impl OrderSubmitter for TxCache {
    type Error = TxCacheError;

    async fn submit_order(&self, order: SignedOrder) -> Result<(), Self::Error> {
        self.forward_order(order).await
    }
}

impl OrderSource for TxCache {
    type Error = TxCacheError;

    fn get_orders(&self) -> impl Stream<Item = Result<SignedOrder, Self::Error>> + Send {
        self.stream_orders()
    }
}

impl BundleSubmitter for TxCache {
    type Response = BundleResponse;
    type Error = TxCacheError;

    async fn submit_bundle(&self, bundle: SignetEthBundle) -> Result<Self::Response, Self::Error> {
        self.forward_bundle(bundle).await
    }
}
