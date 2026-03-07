//! Preflight validation checks for orders and fills.
//!
//! This module provides a [`Permit2PreflightExt`] trait that extends any
//! [`Provider`] with methods to validate that orders can be successfully
//! filled before submitting them to the network. It checks:
//! - Token balances are sufficient
//! - ERC20 approvals are in place for Permit2
//! - Permit2 nonces haven't been consumed

use std::future::Future;

use alloy::{
    primitives::{Address, U256},
    providers::Provider,
};
use futures_util::future::try_join_all;
use signet_types::{SignedOrder, UnsignedOrder, PERMIT2_ADDRESS};
use thiserror::Error;

use crate::OrdersAndFills;

/// Boxed future type alias used for concurrent preflight checks.
type CheckFut<'a> = std::pin::Pin<Box<dyn Future<Output = Result<(), PreflightError>> + Send + 'a>>;

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

/// Convert a nonce to bitmap position (word position and bit position within the word).
fn nonce_to_bitmap_position(nonce: U256) -> (U256, u8) {
    let word_pos = nonce >> 8;
    let bit_pos = (nonce & U256::from(0xFF)).saturating_to::<u8>();
    (word_pos, bit_pos)
}

/// Extension trait that adds Permit2 preflight validation to any [`Provider`].
///
/// # Example
///
/// ```ignore
/// use signet_orders::Permit2PreflightExt;
///
/// let result = provider.preflight_signed_order(&signed_order).await?;
/// ```
pub trait Permit2PreflightExt {
    /// Validate all preflight conditions for a [`SignedOrder`].
    ///
    /// Checks token balances, ERC20 approvals, and Permit2 nonce for each
    /// permitted token in the order. The owner address and nonce are extracted
    /// from the order's permit data.
    fn preflight_signed_order(
        &self,
        order: &SignedOrder,
    ) -> impl Future<Output = Result<(), PreflightError>> + Send;

    /// Validate preflight conditions for an [`UnsignedOrder`].
    ///
    /// Checks token balances and ERC20 approvals for each input token.
    /// Since unsigned orders do not yet have a finalized nonce, the nonce
    /// check is skipped.
    fn preflight_unsigned_order(
        &self,
        order: &UnsignedOrder<'_>,
        user: Address,
    ) -> impl Future<Output = Result<(), PreflightError>> + Send;

    /// Validate preflight conditions for all orders in an [`OrdersAndFills`].
    ///
    /// Runs preflight checks for every [`SignedOrder`] concurrently.
    fn preflight_orders_and_fills(
        &self,
        orders_and_fills: &OrdersAndFills,
    ) -> impl Future<Output = Result<(), PreflightError>> + Send;
}

impl<P: Provider> Permit2PreflightExt for P {
    async fn preflight_signed_order(&self, order: &SignedOrder) -> Result<(), PreflightError> {
        let permit = order.permit();
        let owner = permit.owner;
        let nonce = permit.permit.nonce;

        let mut checks: Vec<CheckFut<'_>> = Vec::new();

        for tp in &permit.permit.permitted {
            checks.push(Box::pin(check_token_balance(self, tp.token, owner, tp.amount)));
            checks.push(Box::pin(check_erc20_approval(self, tp.token, owner, tp.amount)));
        }
        checks.push(Box::pin(check_permit2_nonce(self, owner, nonce)));

        try_join_all(checks).await?;
        Ok(())
    }

    async fn preflight_unsigned_order(
        &self,
        order: &UnsignedOrder<'_>,
        user: Address,
    ) -> Result<(), PreflightError> {
        let checks: Vec<CheckFut<'_>> = order
            .inputs()
            .iter()
            .flat_map(|input| {
                let token = input.token;
                let amount = input.amount;
                [
                    Box::pin(check_token_balance(self, token, user, amount)) as CheckFut<'_>,
                    Box::pin(check_erc20_approval(self, token, user, amount)),
                ]
            })
            .collect();

        try_join_all(checks).await?;
        Ok(())
    }

    async fn preflight_orders_and_fills(
        &self,
        orders_and_fills: &OrdersAndFills,
    ) -> Result<(), PreflightError> {
        let checks: Vec<_> = orders_and_fills
            .orders()
            .iter()
            .map(|order| self.preflight_signed_order(order))
            .collect();

        try_join_all(checks).await?;
        Ok(())
    }
}

/// Check if the user has sufficient balance of a token.
async fn check_token_balance(
    provider: &impl Provider,
    token: Address,
    user: Address,
    required_amount: U256,
) -> Result<(), PreflightError> {
    let erc20 = IERC20::new(token, provider);
    let balance = erc20.balanceOf(user).call().await?;

    if balance < required_amount {
        return Err(PreflightError::InsufficientBalance { have: balance, need: required_amount });
    }

    Ok(())
}

/// Check if the user has approved sufficient allowance to Permit2.
async fn check_erc20_approval(
    provider: &impl Provider,
    token: Address,
    user: Address,
    required_amount: U256,
) -> Result<(), PreflightError> {
    let erc20 = IERC20::new(token, provider);
    let allowance = erc20.allowance(user, PERMIT2_ADDRESS).call().await?;

    if allowance < required_amount {
        return Err(PreflightError::InsufficientAllowance {
            have: allowance,
            need: required_amount,
        });
    }

    Ok(())
}

/// Check if a Permit2 nonce has been consumed.
async fn check_permit2_nonce(
    provider: &impl Provider,
    user: Address,
    nonce: U256,
) -> Result<(), PreflightError> {
    let (word_pos, bit_pos) = nonce_to_bitmap_position(nonce);

    let permit2 = IPermit2::new(PERMIT2_ADDRESS, provider);
    let bitmap = permit2.nonceBitmap(user, word_pos).call().await?;

    // Check if the bit is set (nonce consumed)
    if bitmap & (U256::from(1) << bit_pos) != U256::ZERO {
        return Err(PreflightError::NonceConsumed { word_pos, bit_pos });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::uint;

    #[test]
    fn test_nonce_to_bitmap_position() {
        assert_eq!(nonce_to_bitmap_position(U256::ZERO), (U256::ZERO, 0));
        assert_eq!(nonce_to_bitmap_position(U256::from(255)), (U256::ZERO, 255));
        assert_eq!(nonce_to_bitmap_position(U256::from(256)), (U256::from(1), 0));
        assert_eq!(nonce_to_bitmap_position(U256::from(511)), (U256::from(1), 255));
        assert_eq!(
            nonce_to_bitmap_position(U256::from(0x0123456789ABCDEFu64)),
            (U256::from(0x0123456789ABCDu64), 0xEF)
        );
    }

    #[test]
    fn test_preflight_errors() {
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
        let (word_pos, bit_pos) = nonce_to_bitmap_position(U256::ZERO);
        assert_eq!(word_pos, U256::ZERO);
        assert_eq!(bit_pos, 0);

        let (word_pos, bit_pos) = nonce_to_bitmap_position(U256::from(u64::MAX));
        assert_eq!(word_pos, U256::from(u64::MAX >> 8));
        assert_eq!(bit_pos, 255);

        let (word_pos_255, bit_pos_255) = nonce_to_bitmap_position(U256::from(255));
        let (word_pos_256, bit_pos_256) = nonce_to_bitmap_position(U256::from(256));
        assert_eq!(word_pos_255, U256::ZERO);
        assert_eq!(bit_pos_255, 255);
        assert_eq!(word_pos_256, U256::from(1));
        assert_eq!(bit_pos_256, 0);
    }

    #[test]
    fn test_permit2_address_matches_types() {
        assert_eq!(
            PERMIT2_ADDRESS,
            alloy::primitives::address!("0x000000000022D473030F116dDEE9F6B43aC78BA3")
        );
    }
}
