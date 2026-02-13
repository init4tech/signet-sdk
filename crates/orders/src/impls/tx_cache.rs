use crate::{BundleSubmitter, OrderSource, OrderSubmitter};
use futures_util::future::Either;
use futures_util::stream::{self, Stream, StreamExt};
use signet_bundle::SignetEthBundle;
use signet_tx_cache::{types::BundleReceipt, TxCache, TxCacheError};
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
        stream::unfold(Some(None), move |cursor| async move {
            let cursor = cursor?;

            match TxCache::get_orders(self, cursor).await {
                Ok(response) => {
                    let (inner, next_cursor) = response.into_parts();
                    let orders = stream::iter(inner.orders).map(Ok);
                    Some((Either::Left(orders), next_cursor.map(Some)))
                }
                Err(error) => Some((Either::Right(stream::once(async { Err(error) })), None)),
            }
        })
        .flatten()
    }
}

impl BundleSubmitter for TxCache {
    type Response = BundleReceipt;
    type Error = TxCacheError;

    async fn submit_bundle(&self, bundle: SignetEthBundle) -> Result<Self::Response, Self::Error> {
        self.forward_bundle(bundle).await
    }
}
