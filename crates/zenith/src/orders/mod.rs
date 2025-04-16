mod agg;
pub use agg::AggregateOrders;

mod signed;
pub use signed::{SignedFill, SignedOrder, SignedPermitError};

use crate::HostOrders::HostOrdersInstance;
use alloy::{network::Network, providers::Provider};

impl<P, N> HostOrdersInstance<(), P, N>
where
    P: Provider<N>,
    N: Network,
{
    /// Preflight a signed order to see if the transaction would succeed.
    /// NOTE: Take care with the rpc endpoint used for this. SignedFills should remain private.
    pub async fn try_fill(&self, fill: SignedFill) -> Result<(), alloy::contract::Error> {
        self.fillPermit2(fill.outputs, fill.permit).call().await.map(drop)
    }
}
