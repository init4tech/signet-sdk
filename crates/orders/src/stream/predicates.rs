//! Ready-made predicates for [`OrderStreamExt::filter_orders`].
//!
//! Each function returns an `impl Fn(&SignedOrder) -> bool` that can be passed directly to
//! `filter_orders`; see that method for guidance on captured state in predicates.
//!
//! Predicates that scan a multi-element field (`has_output_*`, `has_input_token`) match if
//! *any* element satisfies the condition. An order whose outputs span multiple chains is
//! therefore retained when filtering by any one of those chain IDs, and likewise for
//! recipients. They are "has any matching element" filters, not "every element matches"
//! filters. `has_output_token` additionally requires the chain ID to match on the same
//! output, since an `Address` can refer to entirely different ERC20 contracts across chains.
//!
//! Compose them by capturing the predicates once and combining the results in a closure - e.g.
//!
//! ```
//! # use alloy::primitives::Address;
//! # use futures_util::stream;
//! # use signet_orders::OrderStreamExt;
//! # use signet_orders::stream::predicates::{not_expired_at, has_input_token, has_output_token};
//! # use signet_types::SignedOrder;
//! # let host_chain = 1u32;
//! # let usdc = Address::repeat_byte(0xaa);
//! # let weth = Address::repeat_byte(0xbb);
//! # let now = 1_700_000_000;
//! # let stream = stream::empty::<Result<SignedOrder, &'static str>>();
//! let alive = not_expired_at(move || now);
//! let from_usdc = has_input_token(usdc);
//! let to_weth = has_output_token(host_chain, weth);
//! let _filtered = stream.filter_orders(move |order| {
//!     alive(order) && from_usdc(order) && to_weth(order)
//! });
//! ```

#[cfg(doc)]
use super::OrderStreamExt;
use alloy::primitives::Address;
use signet_types::SignedOrder;
#[cfg(doc)]
use signet_zenith::RollupOrders::Output;

/// Match orders whose permit deadline is at or after `cutoff()` (i.e. not yet expired).
///
/// `cutoff` is invoked once per `Ok` stream item (the predicate is not called on `Err`s), so it
/// can advance as the stream is consumed. The returned value is seconds since the unix epoch,
/// the same units as the permit `deadline`. Typically pass the earliest fill timestamp (e.g.
/// current time plus a block-lead allowance) so any order that cannot land before its deadline
/// is dropped. Boundary semantics are defined by [`SignedOrder::is_expired_at`].
///
/// For a snapshot/single-request flow with a static cutoff, pass `|| cutoff`. To advance a
/// counter across calls, use an interior-mutability source (e.g. `AtomicU64`).
pub fn not_expired_at(cutoff: impl Fn() -> u64) -> impl Fn(&SignedOrder) -> bool {
    move |order| !order.is_expired_at(cutoff())
}

/// Match orders that have at least one [`Output`] targeting `chain_id`.
pub fn has_output_chain(chain_id: u32) -> impl Fn(&SignedOrder) -> bool {
    move |order| order.outputs().iter().any(|output| output.chainId == chain_id)
}

/// Match orders that have at least one [`Output`] paying `token` on `chain_id`.
///
/// The chain ID is part of the match because the same `Address` can refer to entirely
/// different ERC20 contracts on host and rollup.
pub fn has_output_token(chain_id: u32, token: Address) -> impl Fn(&SignedOrder) -> bool {
    move |order| {
        order.outputs().iter().any(|output| output.chainId == chain_id && output.token == token)
    }
}

/// Match orders that have at least one [`Output`] going to `recipient`.
pub fn has_output_recipient(recipient: Address) -> impl Fn(&SignedOrder) -> bool {
    move |order| order.outputs().iter().any(|output| output.recipient == recipient)
}

/// Match orders whose permit2 batch permits `token` as an input.
pub fn has_input_token(token: Address) -> impl Fn(&SignedOrder) -> bool {
    move |order| order.permit().permit.permitted.iter().any(|input| input.token == token)
}

/// Match orders whose permit2 batch `owner` field equals `owner`.
pub fn with_owner(owner: Address) -> impl Fn(&SignedOrder) -> bool {
    move |order| order.permit().owner == owner
}

#[cfg(test)]
mod tests {
    use super::{super::tests::order_with, *};
    use alloy::primitives::{Signature, U256};
    use core::sync::atomic::{AtomicU64, Ordering};
    use signet_zenith::RollupOrders::{
        Output, Permit2Batch, PermitBatchTransferFrom, TokenPermissions,
    };
    use std::sync::Arc;

    #[test]
    fn not_expired_at_matches_validate_boundary() {
        let order = order_with(Address::ZERO, 100, vec![], vec![]);
        assert!(not_expired_at(|| 99)(&order));
        assert!(not_expired_at(|| 100)(&order), "deadline equal to cutoff is still valid");
        assert!(!not_expired_at(|| 101)(&order));

        // Cross-check against `SignedOrder::validate` to lock in the matching boundary.
        order.validate(99).unwrap();
        order.validate(100).unwrap();
        order.validate(101).unwrap_err();
    }

    #[test]
    fn not_expired_at_re_evaluates_each_call() {
        let order = order_with(Address::ZERO, 100, vec![], vec![]);
        let now = Arc::new(AtomicU64::new(99));
        let predicate = {
            let now = Arc::clone(&now);
            not_expired_at(move || now.load(Ordering::Relaxed))
        };

        // Cutoff 99: 100 >= 99 -> alive.
        assert!(predicate(&order));

        // Advance the clock past the deadline; the same predicate must observe the new value.
        now.store(101, Ordering::Relaxed);
        assert!(!predicate(&order));
    }

    #[test]
    fn not_expired_at_saturates_u256_deadline_above_u64_max() {
        // A deadline that overflows u64 must saturate to u64::MAX, so the order is always
        // considered alive against any u64 cutoff. This mirrors `SignedOrder::validate`, which
        // uses `saturating_to::<u64>()`.
        let order = SignedOrder::new(
            Permit2Batch {
                permit: PermitBatchTransferFrom {
                    permitted: vec![],
                    nonce: U256::ZERO,
                    deadline: U256::MAX,
                },
                owner: Address::ZERO,
                signature: Signature::test_signature().as_bytes().into(),
            },
            vec![],
        );

        assert!(not_expired_at(|| 0)(&order));
        assert!(not_expired_at(|| u64::MAX)(&order));
        order.validate(0).unwrap();
        order.validate(u64::MAX).unwrap();
    }

    #[test]
    fn output_predicates_match_any_output() {
        let token_a = Address::from([0xaa; 20]);
        let token_b = Address::from([0xbb; 20]);
        let recipient = Address::from([0xcc; 20]);
        let order = order_with(
            Address::ZERO,
            1,
            vec![],
            vec![
                Output { token: token_a, amount: U256::ZERO, recipient, chainId: 17001 },
                Output { token: token_b, amount: U256::ZERO, recipient: Address::ZERO, chainId: 1 },
            ],
        );

        assert!(has_output_chain(17001)(&order));
        assert!(has_output_chain(1)(&order));
        assert!(!has_output_chain(2)(&order));

        assert!(has_output_token(17001, token_a)(&order));
        assert!(has_output_token(1, token_b)(&order));
        assert!(!has_output_token(1, token_a)(&order), "right token, wrong chain");
        assert!(!has_output_token(17001, token_b)(&order), "right token, wrong chain");
        assert!(!has_output_token(17001, Address::ZERO)(&order));

        assert!(has_output_recipient(recipient)(&order));
        assert!(has_output_recipient(Address::ZERO)(&order));
        assert!(!has_output_recipient(Address::from([0xde; 20]))(&order));
    }

    #[test]
    fn input_token_matches_any_permitted_input() {
        let token_a = Address::from([0xaa; 20]);
        let token_b = Address::from([0xbb; 20]);
        let order = order_with(
            Address::ZERO,
            1,
            vec![
                TokenPermissions { token: token_a, amount: U256::ZERO },
                TokenPermissions { token: token_b, amount: U256::ZERO },
            ],
            vec![],
        );

        assert!(has_input_token(token_a)(&order));
        assert!(has_input_token(token_b)(&order));
        assert!(!has_input_token(Address::ZERO)(&order));
    }

    #[test]
    fn with_owner_matches_permit_owner() {
        let owner = Address::from([0x11; 20]);
        let order = order_with(owner, 1, vec![], vec![]);
        assert!(with_owner(owner)(&order));
        assert!(!with_owner(Address::ZERO)(&order));
    }

    #[test]
    fn output_predicates_reject_order_with_no_outputs() {
        let order = order_with(Address::ZERO, 1, vec![], vec![]);
        assert!(!has_output_chain(0)(&order));
        assert!(!has_output_chain(1)(&order));
        assert!(!has_output_token(0, Address::ZERO)(&order));
        assert!(!has_output_recipient(Address::ZERO)(&order));
    }

    #[test]
    fn input_token_rejects_order_with_no_permitted_inputs() {
        let order = order_with(Address::ZERO, 1, vec![], vec![]);
        assert!(!has_input_token(Address::ZERO)(&order));
    }
}
