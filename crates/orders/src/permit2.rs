//! Permit2 nonce bitmap utilities for checking order fill status.
//!
//! When an order is filled, its Permit2 nonce is consumed on-chain. These utilities query the
//! Permit2 contract's nonce bitmap to determine whether a specific order has been filled.

use alloy::{
    primitives::{address, Address, U256},
    providers::Provider,
    sol,
};
use signet_types::SignedOrder;
use tracing::instrument;

sol! {
    /// Minimal Permit2 binding for querying the nonce bitmap.
    #[sol(rpc)]
    interface IPermit2 {
        function nonceBitmap(address owner, uint256 wordPos) external view returns (uint256);
    }
}

/// The canonical Permit2 contract address (same on all EVM chains).
pub const PERMIT2: Address = address!("000000000022D473030F116dDEE9F6B43aC78BA3");

/// Errors returned by Permit2 nonce queries.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Permit2Error {
    /// The Permit2 contract call failed.
    #[error("Permit2 nonceBitmap call failed")]
    ContractCall(#[source] alloy::contract::Error),
}

/// Check whether a signed order's Permit2 nonce has been consumed on-chain.
///
/// Queries the Permit2 contract's `nonceBitmap` for the order's owner and nonce word, then checks
/// the specific bit corresponding to the order's nonce.
#[instrument(skip_all, fields(order_hash = %order.order_hash()))]
pub async fn is_order_nonce_consumed<P: Provider>(
    provider: &P,
    order: &SignedOrder,
) -> Result<bool, Permit2Error> {
    let owner = order.permit().owner;
    let nonce = order.permit().permit.nonce;
    let word_pos = nonce >> 8;

    let permit2 = IPermit2::new(PERMIT2, provider);
    let bitmap =
        permit2.nonceBitmap(owner, word_pos).call().await.map_err(Permit2Error::ContractCall)?;

    Ok(is_nonce_consumed(bitmap, nonce))
}

/// Returns `true` if the given nonce has been consumed according to the Permit2 bitmap.
///
/// Permit2 stores nonces as a bitmap: the high 248 bits select the word, the low 8 bits select
/// the bit within that word. This function checks the bit for `nonce` within `bitmap`.
pub fn is_nonce_consumed(bitmap: U256, nonce: U256) -> bool {
    let bit_pos = nonce & U256::from(0xFF);
    (bitmap >> bit_pos) & U256::from(1) != U256::ZERO
}

/// These tests are based on [the canonical tests].
///
/// [the canonical tests]: https://github.com/Uniswap/permit2/blob/main/test/NonceBitmap.t.sol#L9
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonce_0_consumed_when_bit_0_set() {
        let bitmap = U256::from(1);
        assert!(is_nonce_consumed(bitmap, U256::from(0)));
    }

    #[test]
    fn nonce_0_not_consumed_when_bit_0_unset() {
        let bitmap = U256::from(0);
        assert!(!is_nonce_consumed(bitmap, U256::from(0)));
    }

    #[test]
    fn nonce_255_consumed_when_last_bit_set() {
        let bitmap = U256::from(1) << 255;
        assert!(is_nonce_consumed(bitmap, U256::from(255)));
    }

    #[test]
    fn nonce_255_not_consumed_when_last_bit_unset() {
        let bitmap = U256::MAX ^ (U256::from(1) << 255);
        assert!(!is_nonce_consumed(bitmap, U256::from(255)));
    }

    #[test]
    fn mid_range_nonce_consumed_among_other_bits() {
        let bitmap = U256::from(0b1111) | (U256::from(1) << 42);
        assert!(is_nonce_consumed(bitmap, U256::from(42)));
    }

    #[test]
    fn mid_range_nonce_not_consumed_when_only_neighbours_set() {
        let bitmap = (U256::from(1) << 41) | (U256::from(1) << 43);
        assert!(!is_nonce_consumed(bitmap, U256::from(42)));
    }

    #[test]
    fn nonce_word_bits_ignored() {
        // Nonce 0x12A (word=1, bit=42). Only the low 8 bits matter for the bitmap check.
        let bitmap = U256::from(1) << 42;
        assert!(is_nonce_consumed(bitmap, U256::from(0x12A)));
    }
}
