use crate::{item::SimIdentifier, CacheError, SimItem};
use alloy::consensus::TxEnvelope;
use core::fmt;
use parking_lot::RwLock;
use signet_bundle::SignetEthBundle;
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

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
        self.inner.read().items.iter().rev().take(n).map(|(k, item)| (*k, item.clone())).collect()
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
        self.inner.read().items.get(&key).cloned()
    }

    /// Remove an item by key.
    pub fn remove(&self, key: u128) -> Option<SimItem> {
        let mut inner = self.inner.write();
        if let Some(item) = inner.items.remove(&key) {
            inner.seen.remove(item.identifier().as_bytes());
            Some(item)
        } else {
            None
        }
    }

    fn add_inner(inner: &mut CacheInner, mut score: u128, item: SimItem, capacity: usize) {
        // Check if we've already seen this item - if so, don't add it
        if !inner.seen.insert(item.identifier_owned()) {
            return;
        }

        // If it has the same score, we decrement (prioritizing earlier items)
        while inner.items.contains_key(&score) && score != 0 {
            score = score.saturating_sub(1);
        }

        if inner.items.len() >= capacity {
            // If we are at capacity, we need to remove the lowest score
            if let Some((_, item)) = inner.items.pop_first() {
                inner.seen.remove(&item.identifier_owned());
            }
        }

        inner.items.insert(score, item.clone());
    }

    /// Add a bundle to the cache.
    pub fn add_bundle(&self, bundle: SignetEthBundle, basefee: u64) -> Result<(), CacheError> {
        if bundle.replacement_uuid().is_none() {
            // If the bundle does not have a replacement UUID, we cannot add it to the cache.
            return Err(CacheError::BundleWithoutReplacementUuid);
        }

        let item = SimItem::try_from(bundle)?;
        let score = item.calculate_total_fee(basefee);

        let mut inner = self.inner.write();
        Self::add_inner(&mut inner, score, item, self.capacity);

        Ok(())
    }

    /// Add an iterator of bundles to the cache. This locks the cache only once
    ///
    /// Bundles added should have a valid replacement UUID. Bundles without a replacement UUID will be skipped.
    pub fn add_bundles<I, Item>(&self, item: I, basefee: u64) -> Result<(), CacheError>
    where
        I: IntoIterator<Item = Item>,
        Item: Into<SignetEthBundle>,
    {
        let mut inner = self.inner.write();

        for item in item.into_iter() {
            let item = item.into();
            let Ok(item) = SimItem::try_from(item) else {
                // Skip invalid bundles
                continue;
            };
            let score = item.calculate_total_fee(basefee);
            Self::add_inner(&mut inner, score, item, self.capacity);
        }

        Ok(())
    }

    /// Add a transaction to the cache.
    pub fn add_tx(&self, tx: TxEnvelope, basefee: u64) {
        let item = SimItem::from(tx);
        let score = item.calculate_total_fee(basefee);

        let mut inner = self.inner.write();
        Self::add_inner(&mut inner, score, item, self.capacity);
    }

    /// Add an iterator of transactions to the cache. This locks the cache only once
    pub fn add_txs<I>(&self, item: I, basefee: u64)
    where
        I: IntoIterator<Item = TxEnvelope>,
    {
        let mut inner = self.inner.write();

        for item in item.into_iter() {
            let item = SimItem::from(item);
            let score = item.calculate_total_fee(basefee);
            Self::add_inner(&mut inner, score, item, self.capacity);
        }
    }

    /// Clean the cache by removing bundles that are not valid in the current
    /// block.
    pub fn clean(&self, block_number: u64, block_timestamp: u64) {
        let mut inner = self.inner.write();

        // Trim to capacity by dropping lower fees.
        while inner.items.len() > self.capacity {
            if let Some((_, item)) = inner.items.pop_first() {
                // Drop the identifier from the seen cache as well.
                inner.seen.remove(item.identifier().as_bytes());
            }
        }

        let CacheInner { ref mut items, ref mut seen } = *inner;

        items.retain(|_, item| {
            // Retain only items that are not bundles or are valid in the current block.
            if let SimItem::Bundle(bundle) = item {
                let should_remove = bundle.bundle.block_number == block_number
                    && bundle.min_timestamp().is_some_and(|ts| ts <= block_timestamp)
                    && bundle.max_timestamp().is_some_and(|ts| ts >= block_timestamp);

                let retain = !should_remove;

                if should_remove {
                    seen.remove(item.identifier().as_bytes());
                }
                retain
            } else {
                true // Non-bundle items are retained
            }
        });
    }

    /// Clear the cache.
    pub fn clear(&self) {
        let mut inner = self.inner.write();
        inner.items.clear();
        inner.seen.clear();
    }
}

/// Internal cache data, meant to be protected by a lock.
struct CacheInner {
    items: BTreeMap<u128, SimItem>,
    seen: HashSet<SimIdentifier<'static>>,
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

#[cfg(test)]
mod test {
    use alloy::primitives::b256;

    use super::*;

    #[test]
    fn test_cache() {
        let items = vec![
            invalid_tx_with_score(100, 1),
            invalid_tx_with_score(100, 2),
            invalid_tx_with_score(100, 3),
        ];

        let cache = SimCache::with_capacity(2);
        cache.add_txs(items.clone(), 0);

        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(300), Some(items[2].clone().into()));
        assert_eq!(cache.get(200), Some(items[1].clone().into()));
        assert_eq!(cache.get(100), None);
    }

    #[test]
    fn overlap_at_zero() {
        let items = vec![
            invalid_tx_with_score_and_hash(
                1,
                1,
                b256!("0xb36a5a0066980e8477d5d5cebf023728d3cfb837c719dc7f3aadb73d1a39f11f"),
            ),
            invalid_tx_with_score_and_hash(
                1,
                1,
                b256!("0x04d3629f341cdcc5f72969af3c7638e106b4b5620594e6831d86f03ea048e68a"),
            ),
            invalid_tx_with_score_and_hash(
                1,
                1,
                b256!("0x0f0b6a85c1ef6811bf86e92a3efc09f61feb1deca9da671119aaca040021598a"),
            ),
        ];

        let cache = SimCache::with_capacity(2);
        cache.add_txs(items.clone(), 0);

        dbg!(&*cache.inner.read());

        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(0), Some(items[2].clone().into()));
        assert_eq!(cache.get(1), Some(items[0].clone().into()));
        assert_eq!(cache.get(2), None);
    }

    fn invalid_tx_with_score(gas_limit: u64, mpfpg: u128) -> alloy::consensus::TxEnvelope {
        let tx = build_alloy_tx(gas_limit, mpfpg);

        TxEnvelope::Eip1559(alloy::consensus::Signed::new_unhashed(
            tx,
            alloy::signers::Signature::test_signature(),
        ))
    }

    fn invalid_tx_with_score_and_hash(
        gas_limit: u64,
        mpfpg: u128,
        hash: alloy::primitives::B256,
    ) -> alloy::consensus::TxEnvelope {
        let tx = build_alloy_tx(gas_limit, mpfpg);

        TxEnvelope::Eip1559(alloy::consensus::Signed::new_unchecked(
            tx,
            alloy::signers::Signature::test_signature(),
            hash,
        ))
    }

    fn build_alloy_tx(gas_limit: u64, mpfpg: u128) -> alloy::consensus::TxEip1559 {
        alloy::consensus::TxEip1559 {
            gas_limit,
            max_priority_fee_per_gas: mpfpg,
            max_fee_per_gas: alloy::consensus::constants::GWEI_TO_WEI as u128,
            ..Default::default()
        }
    }
}
