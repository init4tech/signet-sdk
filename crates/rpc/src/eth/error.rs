use reth::{
    providers::ProviderError,
    rpc::{eth::filter::EthFilterError, server_types::eth::EthApiError},
};

/// Errors that can occur when interacting with the `eth_` namespace.
#[derive(Debug, thiserror::Error)]
pub enum EthError {
    /// Provider error: [`ProviderError`].
    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),
    /// Filter error [`EthFilterError`].
    #[error("Filter error: {0}")]
    Filter(#[from] EthFilterError),
    /// Eth API error: [`EthApiError`].
    #[error("Eth API error: {0}")]
    Rpc(#[from] EthApiError),
}

impl EthError {
    /// Turn into a string by value, allows for `.map_err(EthError::to_string)`
    /// to be used.
    pub fn into_string(self) -> String {
        ToString::to_string(&self)
    }
}
