use signet_types::MarketError;
use std::fmt::Debug;
use trevm::{
    revm::{primitives::EVMError, Database},
    BundleError,
};

/// Errors that can occur when running a bundle on the Signet EVM.
#[derive(thiserror::Error)]
pub enum SignetBundleError<Db: Database> {
    /// A primitive [`BundleError`] error ocurred.
    #[error(transparent)]
    BundleError(#[from] BundleError<Db>),
    /// A [`MarketError`] ocurred.
    #[error(transparent)]
    MarketError(#[from] MarketError),
}

impl<Db: Database> Debug for SignetBundleError<Db> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignetBundleError::BundleError(e) => write!(f, "BundleError({:?})", e),
            SignetBundleError::MarketError(e) => write!(f, "MarketError({:?})", e),
        }
    }
}

impl<Db: Database> From<EVMError<Db::Error>> for SignetBundleError<Db> {
    fn from(e: EVMError<Db::Error>) -> Self {
        SignetBundleError::BundleError(BundleError::EVMError { inner: e })
    }
}

impl<Db: Database> SignetBundleError<Db> {
    /// Instantiate a new [`SignetBundleError`] from a [`Database::Error`].
    pub const fn evm_db(e: Db::Error) -> Self {
        SignetBundleError::BundleError(BundleError::EVMError { inner: EVMError::Database(e) })
    }
}
