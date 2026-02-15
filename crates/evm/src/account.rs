//! Ethereum account state.

use alloy::primitives::{B256, KECCAK256_EMPTY, U256};
use trevm::revm::state::AccountInfo;

/// An Ethereum account.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Account {
    /// Account nonce.
    pub nonce: u64,
    /// Account balance.
    pub balance: U256,
    /// Hash of the account's bytecode.
    pub bytecode_hash: Option<B256>,
}

impl Account {
    /// Whether the account has bytecode.
    pub const fn has_bytecode(&self) -> bool {
        self.bytecode_hash.is_some()
    }

    /// After `SpuriousDragon` empty account is defined as account with nonce == 0 && balance == 0
    /// && bytecode = None (or hash is [`KECCAK256_EMPTY`]).
    pub fn is_empty(&self) -> bool {
        self.nonce == 0
            && self.balance.is_zero()
            && self.bytecode_hash.is_none_or(|hash| hash == KECCAK256_EMPTY)
    }

    /// Returns an account bytecode's hash.
    /// In case of no bytecode, returns [`KECCAK256_EMPTY`].
    pub fn get_bytecode_hash(&self) -> B256 {
        self.bytecode_hash.unwrap_or(KECCAK256_EMPTY)
    }

    /// Extracts the account information from a [`revm::state::Account`]
    ///
    /// [`revm::state::Account`]: trevm::revm::state::Account
    pub fn from_revm_account(revm_account: &trevm::revm::state::Account) -> Self {
        Self {
            balance: revm_account.info.balance,
            nonce: revm_account.info.nonce,
            bytecode_hash: revm_account
                .info
                .is_empty_code_hash()
                .then_some(revm_account.info.code_hash),
        }
    }
}

impl From<trevm::revm::state::Account> for Account {
    fn from(value: trevm::revm::state::Account) -> Self {
        Self::from_revm_account(&value)
    }
}

impl From<AccountInfo> for Account {
    fn from(revm_acc: AccountInfo) -> Self {
        Self::from(&revm_acc)
    }
}

impl From<&AccountInfo> for Account {
    fn from(revm_acc: &AccountInfo) -> Self {
        Self {
            balance: revm_acc.balance,
            nonce: revm_acc.nonce,
            bytecode_hash: (!revm_acc.is_empty_code_hash()).then_some(revm_acc.code_hash),
        }
    }
}
