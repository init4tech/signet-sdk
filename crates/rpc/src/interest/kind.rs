use crate::interest::{filters::FilterOutput, subs::SubscriptionBuffer};
use alloy::{
    consensus::BlockHeader,
    rpc::types::{Filter, Header, Log},
};
use reth::{
    providers::CanonStateNotification,
    rpc::{server_types::eth::logs_utils::log_matches_filter, types::FilteredParams},
};
use std::collections::VecDeque;

/// The different kinds of filters that can be created.
///
/// Pending tx filters are not supported by Signet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterestKind {
    Log(Box<Filter>),
    Block,
}

impl InterestKind {
    /// True if this is a log filter.
    pub const fn is_filter(&self) -> bool {
        matches!(self, Self::Log(_))
    }

    /// True if this is a block filter.
    pub const fn is_block(&self) -> bool {
        matches!(self, Self::Block)
    }

    /// Fallible cast to a filter.
    pub const fn as_filter(&self) -> Option<&Filter> {
        match self {
            Self::Log(f) => Some(f),
            _ => None,
        }
    }

    fn apply_block(&self, notif: &CanonStateNotification) -> SubscriptionBuffer {
        notif
            .committed()
            .blocks_iter()
            .map(|b| Header {
                hash: b.hash(),
                inner: b.clone_header(),
                total_difficulty: None,
                size: None,
            })
            .collect()
    }

    fn apply_filter(&self, notif: &CanonStateNotification) -> SubscriptionBuffer {
        let filter = self.as_filter().unwrap();

        // NB: borrowing OUTSIDE the top-level closure prevents this value from
        // being moved into the closure, which would result in the inner
        // closures violating borrowing rules.
        let filter_params = &FilteredParams::new(Some(filter.clone()));

        let address_filter = FilteredParams::address_filter(&filter.address);
        let topics_filter = FilteredParams::topics_filter(&filter.topics);

        notif
            .committed()
            .blocks_and_receipts()
            .filter(|(block, _)| {
                let bloom = block.header().logs_bloom();
                FilteredParams::matches_address(bloom, &address_filter)
                    && FilteredParams::matches_topics(bloom, &topics_filter)
            })
            .flat_map(move |(block, receipts)| {
                let block_num_hash = block.num_hash();

                receipts.iter().enumerate().flat_map(move |(transaction_index, receipt)| {
                    let transaction_hash = *block.body().transactions[transaction_index].hash();

                    receipt.logs.iter().enumerate().filter_map(move |(log_index, log)| {
                        if log_matches_filter(block_num_hash, log, filter_params) {
                            Some(Log {
                                inner: log.clone(),
                                block_hash: Some(block_num_hash.hash),
                                block_number: Some(block_num_hash.number),
                                block_timestamp: Some(block.header().timestamp()),
                                transaction_hash: Some(transaction_hash),
                                transaction_index: Some(transaction_index as u64),
                                log_index: Some(log_index as u64),
                                removed: false,
                            })
                        } else {
                            None
                        }
                    })
                })
            })
            .collect()
    }

    /// Apply the filter to a [`CanonStateNotification`]
    pub fn filter_notification_for_sub(
        &self,
        notif: &CanonStateNotification,
    ) -> SubscriptionBuffer {
        if self.is_block() {
            self.apply_block(notif)
        } else {
            self.apply_filter(notif)
        }
    }

    /// Return an empty output of the same kind as this filter.
    pub const fn empty_output(&self) -> FilterOutput {
        match self {
            Self::Log(_) => FilterOutput::Log(VecDeque::new()),
            Self::Block => FilterOutput::Block(VecDeque::new()),
        }
    }

    /// Return an empty subscription buffer of the same kind as this filter.
    pub const fn empty_sub_buffer(&self) -> SubscriptionBuffer {
        match self {
            Self::Log(_) => SubscriptionBuffer::Log(VecDeque::new()),
            Self::Block => SubscriptionBuffer::Block(VecDeque::new()),
        }
    }
}
