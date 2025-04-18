use trevm::MIN_TRANSACTION_GAS;

use crate::SimItem;
use core::fmt;
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

/// A cache for the simulator.
///
/// This cache is used to store the items that are being simulated.
#[derive(Clone)]
pub struct SimCache {
    inner: Arc<RwLock<BTreeMap<u128, SimItem>>>,
    capacity: usize,
}

impl fmt::Debug for SimCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SimCache").finish()
    }
}

impl Default for SimCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SimCache {
    /// Create a new `SimCache` instance.
    pub fn new() -> Self {
        Self { inner: Arc::new(RwLock::new(BTreeMap::new())), capacity: 100 }
    }

    /// Create a new `SimCache` instance with a given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self { inner: Arc::new(RwLock::new(BTreeMap::new())), capacity }
    }

    /// Get an iterator over the best items in the cache.
    pub fn read_best(&self, n: usize) -> Vec<(u128, SimItem)> {
        self.inner.read().unwrap().iter().rev().take(n).map(|(k, v)| (*k, v.clone())).collect()
    }

    /// Get the number of items in the cache.
    pub fn len(&self) -> usize {
        self.inner.read().unwrap().len()
    }

    /// True if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.read().unwrap().is_empty()
    }

    /// Get an item by key.
    pub fn get(&self, key: u128) -> Option<SimItem> {
        self.inner.read().unwrap().get(&key).cloned()
    }

    /// Remove an item by key.
    pub fn remove(&self, key: u128) -> Option<SimItem> {
        self.inner.write().unwrap().remove(&key)
    }

    /// Create a new `SimCache` instance.
    pub fn add_item(&self, item: impl Into<SimItem>) {
        let item = item.into();

        // Calculate the total fee for the item.
        let mut score = item.calculate_total_fee();

        // Sanity check. This should never be true
        if score < MIN_TRANSACTION_GAS as u128 {
            return;
        }

        let mut inner = self.inner.write().unwrap();

        // If it has the same score, we decrement (prioritizing earlier items)
        while inner.contains_key(&score) && score != 0 {
            score = score.saturating_sub(1);
        }

        inner.insert(score, item);
        if inner.len() > self.capacity {
            inner.pop_first();
        }
    }

    /// Clean the cache by removing bundles that are not valid in the current
    /// block.
    pub fn clean(&self, block_number: u64, block_timestamp: u64) {
        let mut inner = self.inner.write().unwrap();

        // Trim to capacity by dropping lower fees.
        while inner.len() > self.capacity {
            inner.pop_first();
        }

        inner.retain(|_, value| {
            let SimItem::Bundle(bundle) = value else {
                return true;
            };
            if bundle.bundle.block_number != block_number {
                return false;
            }
            if let Some(timestamp) = bundle.min_timestamp() {
                if timestamp > block_timestamp {
                    return false;
                }
            }
            if let Some(timestamp) = bundle.max_timestamp() {
                if timestamp < block_timestamp {
                    return false;
                }
            }
            true
        })
    }

    /// Clear the cache.
    pub fn clear(&self) {
        let mut inner = self.inner.write().unwrap();
        inner.clear();
    }
}
