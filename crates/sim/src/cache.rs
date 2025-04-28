use crate::SimItem;
use core::fmt;
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock, RwLockWriteGuard},
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

    fn add_inner(
        guard: &mut RwLockWriteGuard<'_, BTreeMap<u128, SimItem>>,
        mut score: u128,
        item: SimItem,
        capacity: usize,
    ) {
        // If it has the same score, we decrement (prioritizing earlier items)
        while guard.contains_key(&score) && score != 0 {
            score = score.saturating_sub(1);
        }

        if guard.len() >= capacity {
            // If we are at capacity, we need to remove the lowest score
            guard.pop_first();
        }

        guard.entry(score).or_insert(item);
    }

    /// Add an item to the cache.
    ///
    /// The basefee is used to calculate an estimated fee for the item.
    pub fn add_item(&self, item: impl Into<SimItem>, basefee: u64) {
        let item = item.into();

        // Calculate the total fee for the item.
        let score = item.calculate_total_fee(basefee);

        let mut inner = self.inner.write().unwrap();

        Self::add_inner(&mut inner, score, item, self.capacity);
    }

    /// Add an iterator of items to the cache. This locks the cache only once
    pub fn add_items<I, Item>(&self, item: I, basefee: u64)
    where
        I: IntoIterator<Item = Item>,
        Item: Into<SimItem>,
    {
        let iter = item.into_iter().map(|item| {
            let item = item.into();
            let score = item.calculate_total_fee(basefee);
            (score, item)
        });

        let mut inner = self.inner.write().unwrap();

        for (score, item) in iter {
            Self::add_inner(&mut inner, score, item, self.capacity);
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::SimItem;

    #[test]
    fn test_cache() {
        let items = vec![
            SimItem::invalid_item_with_score(100, 1),
            SimItem::invalid_item_with_score(100, 2),
            SimItem::invalid_item_with_score(100, 3),
        ];

        let cache = SimCache::with_capacity(2);
        cache.add_items(items, 0);

        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(300), Some(SimItem::invalid_item_with_score(100, 3)));
        assert_eq!(cache.get(200), Some(SimItem::invalid_item_with_score(100, 2)));
        assert_eq!(cache.get(100), None);
    }

    #[test]
    fn overlap_at_zero() {
        let items = vec![
            SimItem::invalid_item_with_score(1, 1),
            SimItem::invalid_item_with_score(1, 1),
            SimItem::invalid_item_with_score(1, 1),
        ];

        let cache = SimCache::with_capacity(2);
        cache.add_items(items, 0);

        dbg!(&*cache.inner.read().unwrap());

        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(0), Some(SimItem::invalid_item_with_score(1, 1)));
        assert_eq!(cache.get(1), Some(SimItem::invalid_item_with_score(1, 1)));
        assert_eq!(cache.get(2), None);
    }
}
