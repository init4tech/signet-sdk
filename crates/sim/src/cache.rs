use crate::{item::SimIdentifier, SimItem};
use core::fmt;
use parking_lot::RwLock;
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

/// Internal cache data, meant to be protected by a lock.
struct CacheInner {
    items: BTreeMap<u128, (SimItem, SimIdentifier)>,
    seen: HashSet<SimIdentifier>,
}

impl fmt::Debug for CacheInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheInner").finish()
    }
}

impl CacheInner {
    fn new() -> Self {
        Self { items: BTreeMap::new(), seen: HashSet::new() }
    }
}

/// A cache for the simulator.
///
/// This cache is used to store the items that are being simulated.
#[derive(Clone)]
pub struct SimCache {
    inner: Arc<RwLock<CacheInner>>,
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
    /// Create a new `SimCache` instance, with a default capacity of `100`.
    pub fn new() -> Self {
        Self { inner: Arc::new(RwLock::new(CacheInner::new())), capacity: 100 }
    }

    /// Create a new `SimCache` instance with a given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self { inner: Arc::new(RwLock::new(CacheInner::new())), capacity }
    }

    /// Get an iterator over the best items in the cache.
    pub fn read_best(&self, n: usize) -> Vec<(u128, SimItem)> {
        self.inner.read().items.iter().rev().take(n).map(|(k, (v, _))| (*k, v.clone())).collect()
    }

    /// Get the number of items in the cache.
    pub fn len(&self) -> usize {
        self.inner.read().items.len()
    }

    /// True if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.read().items.is_empty()
    }

    /// Get an item by key.
    pub fn get(&self, key: u128) -> Option<SimItem> {
        self.inner.read().items.get(&key).map(|(item, _)| item.clone())
    }

    /// Remove an item by key.
    pub fn remove(&self, key: u128) -> Option<SimItem> {
        let mut inner = self.inner.write();
        if let Some((item, identifier)) = inner.items.remove(&key) {
            inner.seen.remove(&identifier);
            Some(item)
        } else {
            None
        }
    }

    fn add_inner(
        inner: &mut CacheInner,
        mut score: u128,
        item: SimItem,
        identifier: SimIdentifier,
        capacity: usize,
    ) {
        // Check if we've already seen this item - if so, don't add it
        if !inner.seen.insert(identifier.clone()) {
            return;
        }

        // If it has the same score, we decrement (prioritizing earlier items)
        while inner.items.contains_key(&score) && score != 0 {
            score = score.saturating_sub(1);
        }

        if inner.items.len() >= capacity {
            // If we are at capacity, we need to remove the lowest score
            if let Some((_, (_, removed_id))) = inner.items.pop_first() {
                inner.seen.remove(&removed_id);
            }
        }

        inner.items.insert(score, (item, identifier));
    }

    /// Add an item to the cache.
    ///
    /// The basefee is used to calculate an estimated fee for the item.
    pub fn add_item(&self, item: impl Into<SimItem>, basefee: u64) {
        let item = item.into();
        let identifier = item.identifier();
        let score = item.calculate_total_fee(basefee);

        let mut inner = self.inner.write();
        Self::add_inner(&mut inner, score, item, identifier, self.capacity);
    }

    /// Add an iterator of items to the cache. This locks the cache only once
    pub fn add_items<I, Item>(&self, item: I, basefee: u64)
    where
        I: IntoIterator<Item = Item>,
        Item: Into<SimItem>,
    {
        let mut inner = self.inner.write();

        for item in item.into_iter() {
            let item = item.into();
            let identifier = item.identifier();
            let score = item.calculate_total_fee(basefee);
            Self::add_inner(&mut inner, score, item, identifier, self.capacity);
        }
    }

    /// Clean the cache by removing bundles that are not valid in the current
    /// block.
    pub fn clean(&self, block_number: u64, block_timestamp: u64) {
        let mut inner = self.inner.write();

        // Trim to capacity by dropping lower fees.
        while inner.items.len() > self.capacity {
            if let Some((_, (_, id))) = inner.items.pop_first() {
                // Drop the identifier from the seen cache as well.
                inner.seen.remove(&id);
            }
        }

        // Collect items to remove to avoid borrow checker issues
        let mut items_to_remove = Vec::new();

        for (score, (value, id)) in &inner.items {
            let SimItem::Bundle(bundle) = value else {
                continue;
            };

            let should_remove = bundle.bundle.block_number != block_number
                || bundle.min_timestamp().is_some_and(|ts| ts > block_timestamp)
                || bundle.max_timestamp().is_some_and(|ts| ts < block_timestamp);

            if should_remove {
                items_to_remove.push((*score, id.clone()));
            }
        }

        // Remove the collected items
        for (score, id) in items_to_remove {
            inner.items.remove(&score);
            inner.seen.remove(&id);
        }
    }

    /// Clear the cache.
    pub fn clear(&self) {
        let mut inner = self.inner.write();
        inner.items.clear();
        inner.seen.clear();
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::SimItem;

    #[test]
    fn test_cache() {
        let items = vec![
            SimItem::invalid_item_with_score_and_hash(100, 1),
            SimItem::invalid_item_with_score_and_hash(100, 2),
            SimItem::invalid_item_with_score_and_hash(100, 3),
        ];

        let cache = SimCache::with_capacity(2);
        cache.add_items(items.clone(), 0);

        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(300), Some(items[2].clone()));
        assert_eq!(cache.get(200), Some(items[1].clone()));
        assert_eq!(cache.get(100), None);
    }

    #[test]
    fn overlap_at_zero() {
        let items = vec![
            SimItem::invalid_item_with_score_and_hash(1, 1),
            SimItem::invalid_item_with_score_and_hash(1, 1),
            SimItem::invalid_item_with_score_and_hash(1, 1),
        ];

        let cache = SimCache::with_capacity(2);
        cache.add_items(items.clone(), 0);

        dbg!(&*cache.inner.read());

        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(0), Some(items[2].clone()));
        assert_eq!(cache.get(1), Some(items[0].clone()));
        assert_eq!(cache.get(2), None);
    }
}
