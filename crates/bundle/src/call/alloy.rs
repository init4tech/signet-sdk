//! This module extends the Alloy provider with the Signet namespace's bundle-related RPC methods.

use alloy::{network::Network, providers::Provider, transports::TransportResult};

use crate::{SignetCallBundle, SignetCallBundleResponse};

/// Signet namespace RPC interface.
#[cfg_attr(target_family = "wasm", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_family = "wasm"), async_trait::async_trait)]
pub trait SignetBundleApi<N: Network = alloy::network::Ethereum>: Send + Sync {
    /// Simulates a bundle of transactions against a specific block and returns
    /// the execution results.
    ///
    /// This is similar to the Flashbots [`eth_callBundle`] endpoint, but includes
    /// Signet-specific fields like aggregate orders and fills in the response.
    ///
    /// [`eth_callBundle`]: https://docs.flashbots.net/flashbots-auction/advanced/rpc-endpoint#eth_callbundle
    async fn call_bundle(
        &self,
        bundle: SignetCallBundle,
    ) -> TransportResult<SignetCallBundleResponse>;
}

#[cfg_attr(target_family = "wasm", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_family = "wasm"), async_trait::async_trait)]
impl<N, P> SignetBundleApi<N> for P
where
    N: Network,
    P: Provider<N>,
{
    async fn call_bundle(
        &self,
        bundle: SignetCallBundle,
    ) -> TransportResult<SignetCallBundleResponse> {
        self.client().request("signet_callBundle", (bundle,)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::network::Ethereum;
    use alloy::providers::RootProvider;

    #[allow(dead_code)]
    const fn assert_impl<T: SignetBundleApi>() {}
    const _: () = assert_impl::<RootProvider<Ethereum>>();
}
