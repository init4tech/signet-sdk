mod agg;
pub use agg::AggregateOrders;

mod signed;
pub use signed::{SignedOrder, SignedOrderError};

use crate::HostOrders::HostOrdersInstance;
use alloy::{network::Network, providers::Provider};

impl<P, N> HostOrdersInstance<(), P, N>
where
    P: Provider<N>,
    N: Network,
{
    /// Preflight a signed order to see if the transaction would succeed.
    pub async fn try_signed_order(&self, order: SignedOrder) -> Result<(), alloy::contract::Error> {
        self.fillPermit2(order.outputs, order.permit).call().await.map(drop)
    }
}
