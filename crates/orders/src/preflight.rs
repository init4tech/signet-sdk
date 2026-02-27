//! Preflight validation checks for orders and fills.
//!
//! This module provides utilities to validate that orders can be successfully filled
//! before submitting them to the network. It checks:
//! - Token balances are sufficient
//! - ERC20 approvals are in place for Permit2
//! - Permit2 nonces haven't been consumed

use alloy::{
    primitives::{Address, U256},
    providers::Provider,
    transports::Transport,
};
use thiserror::Error;

/// Errors that can occur during preflight validation.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PreflightError {
    /// Provider error occurred while checking conditions.
    #[error("provider error: {0}")]
    Provider(#[from] alloy::contract::Error),
    /// Insufficient token balance for the order.
    #[error("insufficient balance: have {have}, need {need}")]
    InsufficientBalance {
        /// Current balance.
        have: U256,
        /// Required balance.
        need: U256,
    },
    /// Insufficient ERC20 allowance for Permit2.
    #[error("insufficient allowance: have {have}, need {need}")]
    InsufficientAllowance {
        /// Current allowance.
        have: U256,
        /// Required allowance.
        need: U256,
    },
    /// Permit2 nonce has already been consumed.
    #[error("nonce already consumed: word_pos={word_pos}, bit_pos={bit_pos}")]
    NonceConsumed {
        /// Word position in nonce bitmap.
        word_pos: U256,
        /// Bit position in the word.
        bit_pos: u8,
    },
}

// ERC20 interface for balance and allowance checks
alloy::sol! {
    #[sol(rpc)]
    interface IERC20 {
        function balanceOf(address account) external view returns (uint256);
        function allowance(address owner, address spender) external view returns (uint256);
    }
}

// Permit2 interface for nonce validation
alloy::sol! {
    #[sol(rpc)]
    interface IPermit2 {
        function nonceBitmap(address owner, uint256 wordPos) external view returns (uint256);
    }
}

/// The canonical Permit2 contract address.
pub const PERMIT2_ADDRESS: Address =
    alloy::primitives::address!("0x000000000022D473030F116dDEE9F6B43aC78BA3");

/// Preflight validator for checking order conditions before submission.
#[derive(Debug, Clone)]
pub struct PreflightChecker<T, P> {
    provider: P,
    permit2_address: Address,
    _phantom: std::marker::PhantomData<T>,
}

impl<T, P> PreflightChecker<T, P>
where
    T: Transport + Clone,
    P: Provider<T> + Clone,
{
    /// Create a new preflight checker with the default Permit2 address.
    pub fn new(provider: P) -> Self {
        Self::with_permit2_address(provider, PERMIT2_ADDRESS)
    }

    /// Create a new preflight checker with a custom Permit2 address.
    pub fn with_permit2_address(provider: P, permit2_address: Address) -> Self {
        Self { provider, permit2_address, _phantom: std::marker::PhantomData }
    }

    /// Check if the user has sufficient balance of a token.
    pub async fn check_token_balance(
        &self,
        token: Address,
        user: Address,
        required_amount: U256,
    ) -> Result<(), PreflightError> {
        let erc20 = IERC20::new(token, &self.provider);
        let balance = erc20.balanceOf(user).call().await?.balanceOf;

        if balance < required_amount {
            return Err(PreflightError::InsufficientBalance {
                have: balance,
                need: required_amount,
            });
        }

        Ok(())
    }

    /// Check if the user has approved sufficient allowance to Permit2.
    pub async fn check_erc20_approval(
        &self,
        token: Address,
        user: Address,
        required_amount: U256,
    ) -> Result<(), PreflightError> {
        let erc20 = IERC20::new(token, &self.provider);
        let allowance = erc20.allowance(user, self.permit2_address).call().await?.allowance;

        if allowance < required_amount {
            return Err(PreflightError::InsufficientAllowance {
                have: allowance,
                need: required_amount,
            });
        }

        Ok(())
    }

    /// Check if a Permit2 nonce has been consumed.
    pub async fn check_permit2_nonce(
        &self,
        user: Address,
        nonce: u64,
    ) -> Result<(), PreflightError> {
        let (word_pos, bit_pos) = nonce_to_bitmap_position(nonce);

        let permit2 = IPermit2::new(self.permit2_address, &self.provider);
        let bitmap = permit2.nonceBitmap(user, word_pos).call().await?.nonceBitmap;

        // Check if the bit is set (nonce consumed)
        if bitmap & (U256::from(1) << bit_pos) != U256::ZERO {
            return Err(PreflightError::NonceConsumed { word_pos, bit_pos });
        }

        Ok(())
    }

    /// Check all preflight conditions for a token transfer.
    pub async fn check_all(
        &self,
        token: Address,
        user: Address,
        amount: U256,
        nonce: u64,
    ) -> Result<(), PreflightError> {
        // Check balance
        self.check_token_balance(token, user, amount).await?;

        // Check approval
        self.check_erc20_approval(token, user, amount).await?;

        // Check nonce
        self.check_permit2_nonce(user, nonce).await?;

        Ok(())
    }
}

/// Convert a nonce to bitmap position (word position and bit position within the word).
fn nonce_to_bitmap_position(nonce: u64) -> (U256, u8) {
    let word_pos = U256::from(nonce >> 8); // Upper 56 bits
    let bit_pos = (nonce & 0xFF) as u8; // Lower 8 bits
    (word_pos, bit_pos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::{
        primitives::uint,
        providers::{Provider, ProviderBuilder},
    };

    #[test]
    fn test_nonce_to_bitmap_position() {
        // Test some example nonces
        assert_eq!(nonce_to_bitmap_position(0), (U256::ZERO, 0));
        assert_eq!(nonce_to_bitmap_position(255), (U256::ZERO, 255));
        assert_eq!(nonce_to_bitmap_position(256), (U256::from(1), 0));
        assert_eq!(nonce_to_bitmap_position(511), (U256::from(1), 255));
        assert_eq!(
            nonce_to_bitmap_position(0x0123456789ABCDEF),
            (U256::from(0x0123456789ABCD), 0xEF)
        );
    }

    #[tokio::test]
    async fn test_preflight_checker_creation() {
        // Test that we can create a PreflightChecker with the default Permit2 address
        let provider = ProviderBuilder::new().on_builtin("http://localhost:8545").await.unwrap();
        let checker = PreflightChecker::new(provider.clone());
        assert_eq!(checker.permit2_address, PERMIT2_ADDRESS);

        // Test with custom address
        let custom_address = Address::random();
        let checker = PreflightChecker::with_permit2_address(provider, custom_address);
        assert_eq!(checker.permit2_address, custom_address);
    }

    #[test]
    fn test_preflight_errors() {
        // Test error display messages
        let insufficient_balance =
            PreflightError::InsufficientBalance { have: uint!(100_U256), need: uint!(200_U256) };
        assert!(insufficient_balance.to_string().contains("insufficient balance"));
        assert!(insufficient_balance.to_string().contains("100"));
        assert!(insufficient_balance.to_string().contains("200"));

        let insufficient_allowance =
            PreflightError::InsufficientAllowance { have: uint!(50_U256), need: uint!(100_U256) };
        assert!(insufficient_allowance.to_string().contains("insufficient allowance"));

        let nonce_consumed = PreflightError::NonceConsumed { word_pos: uint!(1_U256), bit_pos: 42 };
        assert!(nonce_consumed.to_string().contains("nonce already consumed"));
        assert!(nonce_consumed.to_string().contains("word_pos=1"));
        assert!(nonce_consumed.to_string().contains("bit_pos=42"));
    }

    #[test]
    fn test_bitmap_position_edge_cases() {
        // Test edge cases for bitmap position calculation

        // Minimum value
        let (word_pos, bit_pos) = nonce_to_bitmap_position(u64::MIN);
        assert_eq!(word_pos, U256::ZERO);
        assert_eq!(bit_pos, 0);

        // Maximum value
        let (word_pos, bit_pos) = nonce_to_bitmap_position(u64::MAX);
        assert_eq!(word_pos, U256::from(u64::MAX >> 8));
        assert_eq!(bit_pos, 255);

        // Test bit position boundary (255 -> 0)
        let (word_pos_255, bit_pos_255) = nonce_to_bitmap_position(255);
        let (word_pos_256, bit_pos_256) = nonce_to_bitmap_position(256);
        assert_eq!(word_pos_255, U256::ZERO);
        assert_eq!(bit_pos_255, 255);
        assert_eq!(word_pos_256, U256::from(1));
        assert_eq!(bit_pos_256, 0);
    }

    #[test]
    fn test_constants() {
        // Verify the Permit2 address is correct
        assert_eq!(
            PERMIT2_ADDRESS,
            alloy::primitives::address!("0x000000000022D473030F116dDEE9F6B43aC78BA3")
        );
    }
}
