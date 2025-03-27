use crate::interest::InterestKind;
use alloy::{
    primitives::{B256, U64},
    rpc::types::{Filter, Log},
};
use dashmap::{mapref::one::RefMut, DashMap};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Weak,
    },
    time::{Duration, Instant},
};
use tracing::trace;

type FilterId = U64;

/// Either type for filter outputs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(untagged)]
pub enum Either {
    /// Log
    Log(Log),
    /// Block hash
    Block(B256),
}

/// The output of a filter.
///
/// This will be either a list of logs or a list of block hashes. Pending tx
/// filters are not supported by Signet. For convenience, there is a special
/// variant for empty results.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(untagged)]
pub enum FilterOutput {
    /// Empty output. Holds a `[(); 0]` to make sure it serializes as an empty
    /// array.
    Empty([(); 0]),
    /// Logs
    Log(VecDeque<Log>),
    /// Block hashes
    Block(VecDeque<B256>),
}

impl FilterOutput {
    /// Create an empty filter output.
    pub const fn empty() -> Self {
        Self::Empty([])
    }

    /// True if this is an empty filter output.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The length of this filter output.
    pub fn len(&self) -> usize {
        match self {
            Self::Empty(_) => 0,
            Self::Log(logs) => logs.len(),
            Self::Block(blocks) => blocks.len(),
        }
    }

    /// Extend this filter output with another.
    ///
    /// # Panics
    ///
    /// If the two filter outputs are of different types.
    pub fn extend(&mut self, other: Self) {
        match (self, other) {
            // If we're a log, we can extend with other logs
            (Self::Log(ref mut logs), Self::Log(other_logs)) => logs.extend(other_logs),
            // If we're a block, we can extend with other blocks
            (Self::Block(ref mut blocks), Self::Block(other_blocks)) => blocks.extend(other_blocks),
            // Extending with empty is a noop
            (_, Self::Empty(_)) => (),
            // If we're empty, just take the other value
            (this @ Self::Empty(_), other) => *this = other,
            // This will occur when trying to mix log and block outputs
            _ => panic!("attempted to mix log and block outputs"),
        }
    }

    /// Pop a value from the front of the filter output.
    pub fn pop_front(&mut self) -> Option<Either> {
        match self {
            Self::Log(logs) => logs.pop_front().map(Either::Log),
            Self::Block(blocks) => blocks.pop_front().map(Either::Block),
            Self::Empty(_) => None,
        }
    }
}

impl From<Vec<B256>> for FilterOutput {
    fn from(block_hashes: Vec<B256>) -> Self {
        Self::Block(block_hashes.into())
    }
}

impl From<Vec<Log>> for FilterOutput {
    fn from(logs: Vec<Log>) -> Self {
        Self::Log(logs.into())
    }
}

impl FromIterator<Log> for FilterOutput {
    fn from_iter<T: IntoIterator<Item = Log>>(iter: T) -> Self {
        let inner: VecDeque<_> = iter.into_iter().collect();
        if inner.is_empty() {
            Self::empty()
        } else {
            Self::Log(inner)
        }
    }
}

impl FromIterator<B256> for FilterOutput {
    fn from_iter<T: IntoIterator<Item = B256>>(iter: T) -> Self {
        let inner: VecDeque<_> = iter.into_iter().collect();
        if inner.is_empty() {
            Self::empty()
        } else {
            Self::Block(inner)
        }
    }
}

/// An active filter.
///
/// This struct records
/// - the filter details
/// - the [`Instant`] at which the filter was last polled
/// - the first block whose contents should be considered by the filter
///
/// These are updated via the [`Self::mark_polled`] method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveFilter {
    next_start_block: u64,
    last_poll_time: Instant,
    kind: InterestKind,
}

impl core::fmt::Display for ActiveFilter {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "ActiveFilter {{ next_start_block: {}, ms_since_last_poll: {}, kind: {:?} }}",
            self.next_start_block,
            self.last_poll_time.elapsed().as_millis(),
            self.kind
        )
    }
}

impl ActiveFilter {
    /// True if this is a log filter.
    pub(crate) const fn is_filter(&self) -> bool {
        self.kind.is_filter()
    }

    /// True if this is a block filter.
    pub(crate) const fn is_block(&self) -> bool {
        self.kind.is_block()
    }

    /// Fallible cast to a filter.
    pub(crate) const fn as_filter(&self) -> Option<&Filter> {
        self.kind.as_filter()
    }

    /// Mark the filter as having been polled at the given block.
    pub(crate) fn mark_polled(&mut self, current_block: u64) {
        self.next_start_block = current_block + 1;
        self.last_poll_time = Instant::now();
    }

    /// Get the last block for which the filter was polled.
    pub(crate) const fn next_start_block(&self) -> u64 {
        self.next_start_block
    }

    /// Get the duration since the filter was last polled.
    pub(crate) fn time_since_last_poll(&self) -> Duration {
        self.last_poll_time.elapsed()
    }
}

/// Inner logic for [`FilterManager`].
#[derive(Debug)]
pub(crate) struct FilterManagerInner {
    current_id: AtomicU64,
    filters: DashMap<FilterId, ActiveFilter>,
}

impl FilterManagerInner {
    /// Create a new filter manager.
    fn new() -> Self {
        // Start from 1, as 0 is weird in quantity encoding.
        Self { current_id: AtomicU64::new(1), filters: DashMap::new() }
    }

    /// Get the next filter ID.
    fn next_id(&self) -> FilterId {
        FilterId::from(self.current_id.fetch_add(1, Ordering::Relaxed))
    }

    /// Get a filter by ID.
    pub(crate) fn get_mut(&self, id: FilterId) -> Option<RefMut<'_, U64, ActiveFilter>> {
        self.filters.get_mut(&id)
    }

    fn install(&self, current_block: u64, kind: InterestKind) -> FilterId {
        let id = self.next_id();
        let next_start_block = current_block + 1;
        // discard the result, as we'll not reuse ever.
        let _ = self
            .filters
            .insert(id, ActiveFilter { next_start_block, last_poll_time: Instant::now(), kind });
        id
    }

    /// Install a new log filter.
    pub(crate) fn install_log_filter(&self, current_block: u64, filter: Filter) -> FilterId {
        self.install(current_block, InterestKind::Log(Box::new(filter)))
    }

    /// Install a new block filter.
    pub(crate) fn install_block_filter(&self, current_block: u64) -> FilterId {
        self.install(current_block, InterestKind::Block)
    }

    /// Uninstall a filter, returning the kind of filter that was uninstalled.
    pub(crate) fn uninstall(&self, id: FilterId) -> Option<(U64, ActiveFilter)> {
        self.filters.remove(&id)
    }

    /// Clean stale filters that have not been polled in a while.
    fn clean_stale(&self, older_than: Duration) {
        self.filters.retain(|_, filter| filter.time_since_last_poll() < older_than);
    }
}

/// Manager for filters.
///
/// The manager tracks active filters, and periodically cleans stale filters.
/// Filters are stored in a [`DashMap`] that maps filter IDs to active filters.
/// Filter IDs are assigned sequentially, starting from 1.
///
/// Calling [`Self::new`] spawns a task that periodically cleans stale filters.
/// This task runs on a separate thread to avoid [`DashMap::retain`] deadlock.
/// See [`DashMap`] documentation for more information.
#[derive(Debug, Clone)]
pub(crate) struct FilterManager {
    inner: Arc<FilterManagerInner>,
}

impl FilterManager {
    /// Create a new filter manager. Spawn a task to clean stale filters.
    pub(crate) fn new(clean_interval: Duration, age_limit: Duration) -> Self {
        let inner = Arc::new(FilterManagerInner::new());
        let manager = Self { inner };
        FilterCleanTask::new(Arc::downgrade(&manager.inner), clean_interval, age_limit).spawn();
        manager
    }
}

impl std::ops::Deref for FilterManager {
    type Target = FilterManagerInner;

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

/// Task to clean up unpolled filters.
///
/// This task runs on a separate thread to avoid [`DashMap::retain`] deadlocks.
#[derive(Debug)]
struct FilterCleanTask {
    manager: Weak<FilterManagerInner>,
    sleep: Duration,
    age_limit: Duration,
}

impl FilterCleanTask {
    /// Create a new filter cleaner task.
    const fn new(manager: Weak<FilterManagerInner>, sleep: Duration, age_limit: Duration) -> Self {
        Self { manager, sleep, age_limit }
    }

    /// Run the task. This task runs on a separate thread, which ensures that
    /// [`DashMap::retain`]'s deadlock condition is not met. See [`DashMap`]
    /// documentation for more information.
    fn spawn(self) {
        std::thread::spawn(move || loop {
            std::thread::sleep(self.sleep);
            trace!("cleaning stale filters");
            match self.manager.upgrade() {
                Some(manager) => manager.clean_stale(self.age_limit),
                None => break,
            }
        });
    }
}

// Some code in this file has been copied and modified from reth
// <https://github.com/paradigmxyz/reth>
// The original license is included below:
//
// The MIT License (MIT)
//
// Copyright (c) 2022-2025 Reth Contributors
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//.
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.
