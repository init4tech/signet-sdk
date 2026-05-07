//! Stream combinators for filtering [`SignedOrder`] streams.
//!
//! [`OrderStreamExt`] adds `filter_orders` to any `Stream<Item = Result<SignedOrder, E>>`.
//! Errors flow through unchanged; only `Ok` items are tested against the predicate.

use futures_util::{future, Stream, StreamExt};
use signet_types::SignedOrder;

pub mod predicates;

/// Stream extension that filters [`SignedOrder`] items by predicate.
///
/// `Err` items pass through unchanged; only `Ok(SignedOrder)` items are tested against the
/// predicate. Items where the predicate returns `false` are discarded.
pub trait OrderStreamExt: Sized {
    /// The error type carried by the underlying stream's `Result` items.
    type Error;

    /// Filter the stream by `predicate`.
    ///
    /// `predicate` is `FnMut`, so it can carry state across calls (e.g. for deduplication). Be
    /// aware that any captured state lives for the lifetime of the returned stream - an unbounded
    /// `HashSet` over a long-running stream will grow without bound, so prefer a bounded structure
    /// (e.g. an LRU) for production use.
    #[must_use = "filter_orders returns a new stream and does nothing unless polled"]
    fn filter_orders<F>(self, predicate: F) -> impl Stream<Item = Result<SignedOrder, Self::Error>>
    where
        F: FnMut(&SignedOrder) -> bool;
}

impl<S, E> OrderStreamExt for S
where
    S: Stream<Item = Result<SignedOrder, E>>,
{
    type Error = E;

    fn filter_orders<F>(self, mut predicate: F) -> impl Stream<Item = Result<SignedOrder, E>>
    where
        F: FnMut(&SignedOrder) -> bool,
    {
        self.filter(move |item| {
            future::ready(match item {
                Ok(order) => predicate(order),
                Err(_) => true,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Address, Signature, U256};
    use core::convert::Infallible;
    use futures_util::stream;
    use signet_zenith::RollupOrders::{
        Output, Permit2Batch, PermitBatchTransferFrom, TokenPermissions,
    };
    use std::collections::HashSet;

    pub(super) fn order_with(
        owner: Address,
        deadline: u64,
        permitted: Vec<TokenPermissions>,
        outputs: Vec<Output>,
    ) -> SignedOrder {
        SignedOrder::new(
            Permit2Batch {
                permit: PermitBatchTransferFrom {
                    permitted,
                    nonce: U256::ZERO,
                    deadline: U256::from(deadline),
                },
                owner,
                signature: Signature::test_signature().as_bytes().into(),
            },
            outputs,
        )
    }

    #[tokio::test]
    async fn filter_orders_drops_failing_predicate_and_passes_errors() {
        let by_deadline = |deadline| order_with(Address::ZERO, deadline, vec![], vec![]);
        let items: Vec<Result<SignedOrder, &'static str>> = vec![
            Ok(by_deadline(1)),
            Err("boom"),
            Ok(by_deadline(10)),
            Err("bang"),
            Ok(by_deadline(100)),
        ];

        let collected: Vec<_> = stream::iter(items)
            .filter_orders(|order| order.permit().permit.deadline > U256::from(5))
            .collect()
            .await;

        assert_eq!(collected.len(), 4);
        assert_eq!(collected[0].as_ref().unwrap_err(), &"boom");
        assert_eq!(collected[1].as_ref().unwrap().permit().permit.deadline, U256::from(10));
        assert_eq!(collected[2].as_ref().unwrap_err(), &"bang");
        assert_eq!(collected[3].as_ref().unwrap().permit().permit.deadline, U256::from(100));
    }

    #[tokio::test]
    async fn filter_orders_can_dedupe_with_stateful_predicate() {
        let owner_a = Address::from([0xa1; 20]);
        let owner_b = Address::from([0xb2; 20]);
        let make = |owner| order_with(owner, 1, vec![], vec![]);
        let items = [make(owner_a), make(owner_b), make(owner_a), make(owner_b)]
            .into_iter()
            .map(Ok::<_, Infallible>);

        let mut seen = HashSet::new();
        let collected: Vec<_> = stream::iter(items)
            .filter_orders(move |order| seen.insert(*order.order_hash()))
            .collect()
            .await;

        assert_eq!(collected.len(), 2);
        assert_eq!(collected[0].as_ref().unwrap(), &make(owner_a));
        assert_eq!(collected[1].as_ref().unwrap(), &make(owner_b));
    }

    #[tokio::test]
    async fn filter_orders_works_with_predicate_helpers() {
        let target = Address::from([0x42; 20]);
        let other = Address::from([0x01; 20]);

        let matching = order_with(
            other,
            1,
            vec![TokenPermissions { token: target, amount: U256::ZERO }],
            vec![],
        );
        let non_matching = order_with(
            other,
            1,
            vec![TokenPermissions { token: other, amount: U256::ZERO }],
            vec![],
        );

        let items: Vec<Result<SignedOrder, &'static str>> = vec![
            Ok(matching.clone()),
            Err("error 1"),
            Ok(non_matching),
            Ok(matching.clone()),
            Err("error 2"),
        ];

        let collected: Vec<_> =
            stream::iter(items).filter_orders(predicates::has_input_token(target)).collect().await;

        assert_eq!(collected.len(), 4);
        assert_eq!(collected[0].as_ref().unwrap(), &matching);
        assert_eq!(collected[1].as_ref().unwrap_err(), &"error 1");
        assert_eq!(collected[2].as_ref().unwrap(), &matching);
        assert_eq!(collected[3].as_ref().unwrap_err(), &"error 2");
    }

    #[tokio::test]
    async fn filter_orders_composes_predicates() {
        let chain_id = 17001u32;
        let token = Address::from([0x42; 20]);
        let other_token = Address::from([0x01; 20]);

        let on_chain =
            |chain| Output { token, amount: U256::ZERO, recipient: Address::ZERO, chainId: chain };
        let with_input = |input| vec![TokenPermissions { token: input, amount: U256::ZERO }];

        let matches_all =
            order_with(Address::ZERO, 100, with_input(token), vec![on_chain(chain_id)]);
        let wrong_input =
            order_with(Address::ZERO, 100, with_input(other_token), vec![on_chain(chain_id)]);
        let wrong_chain = order_with(Address::ZERO, 100, with_input(token), vec![on_chain(1)]);
        let expired = order_with(Address::ZERO, 50, with_input(token), vec![on_chain(chain_id)]);

        let items = [matches_all.clone(), wrong_input, wrong_chain, expired]
            .into_iter()
            .map(Ok::<_, Infallible>);

        let alive = predicates::not_expired_at(|| 100);
        let from_input = predicates::has_input_token(token);
        let to_chain = predicates::has_output_chain(chain_id);

        let collected: Vec<_> = stream::iter(items)
            .filter_orders(move |order| alive(order) && from_input(order) && to_chain(order))
            .collect()
            .await;

        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].as_ref().unwrap(), &matches_all);
    }

    #[tokio::test]
    async fn filter_orders_does_not_invoke_predicate_on_errors() {
        let order = order_with(Address::ZERO, 1, vec![], vec![]);
        let items: Vec<Result<SignedOrder, &'static str>> =
            vec![Err("a"), Ok(order.clone()), Err("b"), Ok(order), Err("c")];

        let mut calls = 0u32;
        let collected: Vec<_> = stream::iter(items)
            .filter_orders(|_| {
                calls += 1;
                true
            })
            .collect()
            .await;

        assert_eq!(calls, 2, "predicate should only run on Ok items");
        assert_eq!(collected.len(), 5, "all items should pass through when predicate returns true");
        assert_eq!(collected[0].as_ref().unwrap_err(), &"a");
        collected[1].as_ref().unwrap();
        assert_eq!(collected[2].as_ref().unwrap_err(), &"b");
        collected[3].as_ref().unwrap();
        assert_eq!(collected[4].as_ref().unwrap_err(), &"c");
    }

    #[tokio::test]
    async fn filter_orders_handles_empty_stream() {
        let items: Vec<Result<SignedOrder, Infallible>> = vec![];
        let collected: Vec<_> = stream::iter(items).filter_orders(|_| true).collect().await;
        assert!(collected.is_empty());
    }

    #[tokio::test]
    async fn filter_orders_handles_all_rejected() {
        let items = (0..3u64).map(|deadline| {
            Ok::<_, Infallible>(order_with(Address::ZERO, deadline, vec![], vec![]))
        });
        let collected: Vec<_> = stream::iter(items).filter_orders(|_| false).collect().await;
        assert!(collected.is_empty());
    }

    #[tokio::test]
    async fn filter_orders_can_be_chained() {
        let token_a = Address::from([0xaa; 20]);
        let token_b = Address::from([0xbb; 20]);
        let with_inputs = |tokens: Vec<Address>| {
            order_with(
                Address::ZERO,
                1,
                tokens
                    .into_iter()
                    .map(|t| TokenPermissions { token: t, amount: U256::ZERO })
                    .collect(),
                vec![],
            )
        };

        let only_a = with_inputs(vec![token_a]);
        let both = with_inputs(vec![token_a, token_b]);
        let only_b = with_inputs(vec![token_b]);

        let items = [only_a, both.clone(), only_b].into_iter().map(Ok::<_, Infallible>);

        let collected: Vec<_> = stream::iter(items)
            .filter_orders(predicates::has_input_token(token_a))
            .filter_orders(predicates::has_input_token(token_b))
            .collect()
            .await;

        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].as_ref().unwrap(), &both);
    }
}
