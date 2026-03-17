//! Preflight validation checks for orders and fills.
//!
//! This module provides a [`Permit2Ext`] trait that extends any
//! [`Provider`] with methods to validate that orders can be successfully
//! filled before submitting them to the network. It checks:
//! - Token balances are sufficient
//! - ERC20 approvals are in place for Permit2
//! - Permit2 nonces haven't been consumed

use crate::OrdersAndFills;
use alloy::{
    primitives::{Address, U256},
    providers::Provider,
};
use futures_util::future::try_join_all;
use signet_constants::contracts::{IPermit2, IERC20};
use signet_types::{SignedOrder, UnsignedOrder, PERMIT2_ADDRESS};
use std::future::Future;
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

/// Convert a nonce to bitmap position (word position and bit position within the word).
fn nonce_to_bitmap_position(nonce: U256) -> (U256, u8) {
    let word_pos = nonce >> 8;
    let bit_pos = (nonce & U256::from(0xFF)).saturating_to::<u8>();
    (word_pos, bit_pos)
}

/// Extension trait that adds Permit2 preflight validation to any [`Provider`].
///
/// Provides low-level checks ([`sufficient_balance`], [`token_approved`],
/// [`nonce_available`]) and high-level order validation methods.
///
/// [`sufficient_balance`]: Permit2Ext::sufficient_balance
/// [`token_approved`]: Permit2Ext::token_approved
/// [`nonce_available`]: Permit2Ext::nonce_available
///
/// # Example
///
/// ```ignore
/// use signet_orders::Permit2Ext;
///
/// provider.check_signed_order(&signed_order).await?;
/// ```
pub trait Permit2Ext {
    /// Check if `user` has at least `amount` of `token`.
    fn sufficient_balance(
        &self,
        token: Address,
        user: Address,
        amount: U256,
    ) -> impl Future<Output = Result<(), PreflightError>> + Send;

    /// Check if `user` has approved at least `amount` of `token` to Permit2.
    fn token_approved(
        &self,
        token: Address,
        user: Address,
        amount: U256,
    ) -> impl Future<Output = Result<(), PreflightError>> + Send;

    /// Check if a Permit2 `nonce` is still available (not yet consumed).
    fn nonce_available(
        &self,
        user: Address,
        nonce: U256,
    ) -> impl Future<Output = Result<(), PreflightError>> + Send;

    /// Validate all preflight conditions for a [`SignedOrder`].
    ///
    /// Checks token balances, ERC20 approvals, and Permit2 nonce for each
    /// permitted token. Runs all checks concurrently.
    fn check_signed_order(
        &self,
        order: &SignedOrder,
    ) -> impl Future<Output = Result<(), PreflightError>> + Send;

    /// Validate preflight conditions for an [`UnsignedOrder`].
    ///
    /// Checks token balances and ERC20 approvals for each input token.
    /// Nonce check is skipped since unsigned orders lack a finalized nonce.
    fn check_unsigned_order(
        &self,
        order: &UnsignedOrder<'_>,
        user: Address,
    ) -> impl Future<Output = Result<(), PreflightError>> + Send;

    /// Validate preflight conditions for all orders in an [`OrdersAndFills`].
    ///
    /// Runs [`check_signed_order`] for every order concurrently.
    ///
    /// [`check_signed_order`]: Permit2Ext::check_signed_order
    fn check_orders_and_fills(
        &self,
        orders_and_fills: &OrdersAndFills,
    ) -> impl Future<Output = Result<(), PreflightError>> + Send;
}

impl<P: Provider> Permit2Ext for P {
    async fn sufficient_balance(
        &self,
        token: Address,
        user: Address,
        amount: U256,
    ) -> Result<(), PreflightError> {
        let balance = IERC20::new(token, self).balanceOf(user).call().await?;
        if balance < amount {
            return Err(PreflightError::InsufficientBalance { have: balance, need: amount });
        }
        Ok(())
    }

    async fn token_approved(
        &self,
        token: Address,
        user: Address,
        amount: U256,
    ) -> Result<(), PreflightError> {
        let allowance = IERC20::new(token, self).allowance(user, PERMIT2_ADDRESS).call().await?;
        if allowance < amount {
            return Err(PreflightError::InsufficientAllowance { have: allowance, need: amount });
        }
        Ok(())
    }

    async fn nonce_available(&self, user: Address, nonce: U256) -> Result<(), PreflightError> {
        let (word_pos, bit_pos) = nonce_to_bitmap_position(nonce);
        let bitmap =
            IPermit2::new(PERMIT2_ADDRESS, self).nonceBitmap(user, word_pos).call().await?;
        if bitmap & (U256::from(1) << bit_pos) != U256::ZERO {
            return Err(PreflightError::NonceConsumed { word_pos, bit_pos });
        }
        Ok(())
    }

    async fn check_signed_order(&self, order: &SignedOrder) -> Result<(), PreflightError> {
        let permit = order.permit();
        let owner = permit.owner;

        let balance_checks = permit
            .permit
            .permitted
            .iter()
            .map(|tp| self.sufficient_balance(tp.token, owner, tp.amount));
        let approval_checks = permit
            .permit
            .permitted
            .iter()
            .map(|tp| self.token_approved(tp.token, owner, tp.amount));

        tokio::try_join!(
            try_join_all(balance_checks),
            try_join_all(approval_checks),
            self.nonce_available(owner, permit.permit.nonce),
        )?;
        Ok(())
    }

    async fn check_unsigned_order(
        &self,
        order: &UnsignedOrder<'_>,
        user: Address,
    ) -> Result<(), PreflightError> {
        let balance_checks = order
            .inputs()
            .iter()
            .map(|input| self.sufficient_balance(input.token, user, input.amount));
        let approval_checks =
            order.inputs().iter().map(|input| self.token_approved(input.token, user, input.amount));

        tokio::try_join!(try_join_all(balance_checks), try_join_all(approval_checks),)?;
        Ok(())
    }

    async fn check_orders_and_fills(
        &self,
        orders_and_fills: &OrdersAndFills,
    ) -> Result<(), PreflightError> {
        try_join_all(orders_and_fills.orders().iter().map(|order| self.check_signed_order(order)))
            .await?;
        Ok(())
    }
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
