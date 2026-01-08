use alloy::eips::eip2718::Eip2718Error;
use signet_types::{MarketError, SignedPermitError};
use trevm::{
    revm::{context::result::EVMError, Database},
    BundleError,
};

/// Errors that can occur while recovering signatures from transactions in
/// bundles.
#[derive(Debug, thiserror::Error)]
pub enum RecoverError {
    /// Bundle is empty. Bundles must contain at least one RU transaction.
    #[error("Bundle must contain at least one RU transaction")]
    EmptyBundle,

    /// Error occurred while decoding the transaction.
    #[error(transparent)]
    Decoding(#[from] Eip2718Error),

    /// Error occurred while recovering the signature.
    #[error(transparent)]
    Recovering(#[from] alloy::consensus::crypto::RecoveryError),
}

/// Decoding error specifying the an error encountered while decoding
/// transactions in a Signet bundle.
#[derive(Debug, thiserror::Error)]
#[error("Failed to decode transaction. Host: {host}, Index: {index}, Error: {inner}")]
pub struct BundleRecoverError {
    /// Error decoding a transaction.
    #[source]
    pub inner: RecoverError,
    /// Whether the transaction was a host transaction.
    pub host: bool,
    /// Index of the transaction in the bundle.
    pub index: usize,
}

impl BundleRecoverError {
    /// Creates a new `BundleRecoverError`.
    pub fn new(inner: impl Into<RecoverError>, host: bool, index: usize) -> Self {
        Self { inner: inner.into(), host, index }
    }
}

/// Errors while running a [`SignetEthBundle`] on the EVM.
#[derive(thiserror::Error)]
pub enum SignetEthBundleError<Db: Database> {
    /// Bundle error.
    #[error(transparent)]
    Bundle(#[from] BundleError<Db>),

    /// SignetPermit error.
    #[error(transparent)]
    SignetPermit(#[from] SignedPermitError),

    /// Contract error.
    #[error(transparent)]
    Contract(#[from] alloy::contract::Error),

    /// Market error.
    #[error(transparent)]
    Market(#[from] MarketError),

    /// Host simulation error.
    #[error("{0}")]
    HostSimulation(&'static str),
}

impl<Db: Database> core::fmt::Debug for SignetEthBundleError<Db> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignetEthBundleError::Bundle(inner) => {
                f.debug_tuple("BundleError").field(inner).finish()
            }
            SignetEthBundleError::SignetPermit(inner) => {
                f.debug_tuple("SignedPermitError").field(inner).finish()
            }
            SignetEthBundleError::Contract(inner) => {
                f.debug_tuple("ContractError").field(inner).finish()
            }
            SignetEthBundleError::Market(inner) => {
                f.debug_tuple("MarketError").field(inner).finish()
            }
            SignetEthBundleError::HostSimulation(msg) => {
                f.debug_tuple("HostSimulationError").field(msg).finish()
            }
        }
    }
}

impl<Db: Database> From<EVMError<Db::Error>> for SignetEthBundleError<Db> {
    fn from(err: EVMError<Db::Error>) -> Self {
        Self::Bundle(BundleError::from(err))
    }
}
