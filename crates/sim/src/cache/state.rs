use alloy::{
    primitives::{Address, U256},
    providers::Provider,
    transports::TransportError,
};
use core::future::Future;
use trevm::revm::database_interface::async_db::DatabaseAsyncRef;

/// Account information including nonce and balance. This is partially modeled
/// after [`revm::AccountInfo`], but only includes the fields we care about.
///
/// [`revm::AccountInfo`]: trevm::revm::state::AccountInfo
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcctInfo {
    /// The account nonce.
    pub nonce: u64,
    /// The account balance.
    pub balance: U256,
    /// Whether the account has deployed code.
    pub has_code: bool,
}

/// A source for nonce and balance information. Exists to simplify type bounds
/// in various places.
pub trait StateSource: Send + Sync {
    /// The error type for state lookups.
    type Error: core::error::Error + Send + 'static;

    /// Get account details for an address.
    fn account_details(
        &self,
        address: &Address,
    ) -> impl Future<Output = Result<AcctInfo, Self::Error>> + Send;

    /// Get the nonce for an address. Returns the NEXT EXPECTED nonce, i.e. `0` for an address that
    /// has never sent a transaction, 1 for an address that has sent exactly one transaction, etc.
    fn nonce(&self, address: &Address) -> impl Future<Output = Result<u64, Self::Error>> + Send {
        async { self.account_details(address).await.map(|info| info.nonce) }
    }

    /// Get the balance for an address.
    fn balance(&self, address: &Address) -> impl Future<Output = Result<U256, Self::Error>> + Send {
        async { self.account_details(address).await.map(|info| info.balance) }
    }

    /// Run an arbitrary check on the account details for an address.
    fn map<T: Send, F: FnOnce(&AcctInfo) -> T + Send>(
        &self,
        address: &Address,
        f: F,
    ) -> impl Future<Output = Result<T, Self::Error>> + Send {
        async { self.account_details(address).await.map(|info| f(&info)) }
    }
}

impl<Db> StateSource for Db
where
    Db: DatabaseAsyncRef + Send + Sync,
    Db::Error: Send + 'static,
{
    type Error = Db::Error;

    async fn account_details(&self, address: &Address) -> Result<AcctInfo, Self::Error> {
        let info = self.basic_async_ref(*address).await?.unwrap_or_default();
        let has_code = info.code_hash() != trevm::revm::primitives::KECCAK_EMPTY;
        Ok(AcctInfo { nonce: info.nonce, balance: info.balance, has_code })
    }
}

/// A wrapper that implements [`StateSource`] for any alloy [`Provider`].
///
/// This allows using an alloy provider as a state source for bundle tx list
/// validation via [`check_bundle_tx_list`].
///
/// [`check_bundle_tx_list`]: crate::check_bundle_tx_list
#[derive(Debug, Clone)]
pub struct ProviderStateSource<P>(pub P);

impl<P: Provider> StateSource for ProviderStateSource<P> {
    type Error = TransportError;

    async fn account_details(&self, address: &Address) -> Result<AcctInfo, Self::Error> {
        let nonce = self.0.get_transaction_count(*address).await?;
        let balance = self.0.get_balance(*address).await?;
        Ok(AcctInfo { nonce, balance, has_code: false })
    }
}
