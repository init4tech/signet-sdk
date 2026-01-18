use alloy::primitives::{Address, U256};

/// Account information including nonce and balance. This is partially modeled
/// after [`revm::AccountInfo`], but only includes the fields we care about.
///
/// [`revm::AccountInfo`]: trevm::revm::state::AccountInfo
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcctInfo {
    pub nonce: u64,
    pub balance: U256,
    pub has_code: bool,
}

/// A source for nonce and balance information. Exists to simplify type bounds
/// in various places.
pub trait StateSource {
    /// The error type for state lookups, usually a database error.
    type Error: core::error::Error + 'static;

    /// Get account details for an address.
    fn account_details(&self, address: &Address) -> Result<AcctInfo, Self::Error>;

    /// Get the nonce for an address. This should return the NEXT EXPECTED
    /// nonce. I.e. `0` for an address that has never sent a transaction, 1 for an address that has sent exactly one transaction, etc.
    fn nonce(&self, address: &Address) -> Result<u64, Self::Error> {
        self.account_details(address).map(|info| info.nonce)
    }

    /// Get the balance for an address.
    fn balance(&self, address: &Address) -> Result<U256, Self::Error> {
        self.account_details(address).map(|info| info.balance)
    }

    /// Run an arbitrary check on the account details for an address.
    fn map<T, F: FnOnce(&AcctInfo) -> T>(&self, address: &Address, f: F) -> Result<T, Self::Error> {
        self.account_details(address).map(|info| f(&info))
    }
}

impl<Db> StateSource for Db
where
    Db: trevm::revm::DatabaseRef<Error: 'static>,
{
    type Error = Db::Error;

    fn account_details(&self, address: &Address) -> Result<AcctInfo, Self::Error> {
        let info = self.basic_ref(*address)?.unwrap_or_default();

        let has_code = info.code_hash() != trevm::revm::primitives::KECCAK_EMPTY;

        Ok(AcctInfo { nonce: info.nonce, balance: info.balance, has_code })
    }
}
