use crate::TxBuilder;
use alloy::{
    network::Network,
    providers::{
        fillers::{FillProvider, TxFiller},
        Provider, SendableTx,
    },
    transports::TransportResult,
};

impl<F, P, N> TxBuilder<N> for FillProvider<F, P, N>
where
    F: TxFiller<N>,
    P: Provider<N>,
    N: Network,
{
    async fn fill(&self, tx: N::TransactionRequest) -> TransportResult<SendableTx<N>> {
        FillProvider::fill(self, tx).await
    }
}
