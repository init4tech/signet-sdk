use crate::cache::{CacheError, SimIdentifier, SimItem, StateSource};
use alloy::consensus::{transaction::Recovered, TxEnvelope};
use core::fmt;
use lru::LruCache;
use parking_lot::RwLock;
use signet_bundle::{RecoveredBundle, SignetEthBundle};
use std::{
    collections::{BTreeMap, HashSet},
    mem::MaybeUninit,
    num::NonZeroUsize,
    ops::Deref,
    sync::Arc,
};

/// A cache for the simulator.
///
/// This cache is used to store the items that are being simulated.
#[derive(Clone)]
pub struct SimCache {
    inner: Arc<RwLock<CacheStore>>,
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
        Self { inner: Arc::new(RwLock::new(CacheStore::new())), capacity: 100 }
    }

    /// Create a new `SimCache` instance with a given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self { inner: Arc::new(RwLock::new(CacheStore::new())), capacity }
    }

    /// Fill a buffer with up to its capacity
    pub fn write_best_to(&self, buf: &mut [MaybeUninit<(u128, SimItem)>]) -> usize {
        let cache = self.inner.read();
        cache.items.iter().rev().zip(buf.iter_mut()).for_each(|((cache_rank, item), slot)| {
            // Cloning the Arc into the MaybeUninit slot
            slot.write((*cache_rank, item.clone()));
        });
        // We wrote the minimum of what was in the cache and the buffer
        std::cmp::min(cache.items.len(), buf.len())
    }

    /// Get an iterator over the best items in the cache.
    pub fn read_best(&self, n: usize) -> Vec<(u128, SimItem)> {
        let mut vec = Vec::with_capacity(n);
        let n = self.write_best_to(vec.spare_capacity_mut());
        // SAFETY: We just wrote n items.
        unsafe { vec.set_len(n) };
        vec
    }

    /// Iter over the best items in the cache, writing only those that pass
    /// preflight validity checks (nonce and initial fee) to the buffer.
    ///
    /// The state sources are used to validate the items against the current
    /// nonce and balance, to prevent simulating invalid items.
    ///
    /// This will additionally remove items that can _never_ be valid from the
    /// cache.
    ///
    /// When an error is encountered, the process stops and the error is
    /// returned. At this point, the buffer may be partially written.
    pub fn write_best_valid_to<S, S2>(
        &self,
        buf: &mut [MaybeUninit<(u128, SimItem)>],
        source: &S,
        host_source: &S2,
    ) -> Result<usize, Box<dyn std::error::Error>>
    where
        S: StateSource,
        S2: StateSource,
    {
        let mut cache = self.inner.upgradable_read();
        let mut slots = buf.iter_mut();
        let start = slots.len();

        let mut never = Vec::new();

        // Traverse the cache in reverse order (best items first), checking
        // each item.
        //
        // Errors are shortcut by `try_for_each`. Passes are written to the
        // buffer, consuming slots. Once no slots are left, the try_for_each
        // returns early.
        let res = cache
            .items
            .iter()
            .rev()
            .map(|(rank, item)| {
                item.check(source, host_source).map(|validity| (validity, rank, item))
            })
            .try_for_each(|result| {
                if slots.len() == 0 {
                    return Ok(());
                }
                let (validity, rank, item) = result?;

                if validity.is_valid_now() {
                    slots.next().expect("checked by len").write((*rank, item.clone()));
                }
                if validity.is_never_valid() {
                    never.push(*rank);
                }

                Ok(())
            })
            .map(|_| start - slots.len());

        cache.with_upgraded(|cache| {
            // Remove never valid items from the cache
            never.iter().for_each(|rank| {
                cache.remove_and_disallow(*rank);
            });
        });

        res
    }

    /// Get up to the `n` best items in the cache that pass preflight validity
    /// checks (nonce and initial fee). The returned vector may be smaller than
    /// `n` if not enough valid items are found.
    ///
    /// This will additionally remove items that can _never_ be valid from the
    /// cache.
    ///
    /// The state sources are used to validate the items against the current
    /// nonce and balance, to prevent simulating invalid items.
    pub fn read_best_valid<S, S2>(
        &self,
        n: usize,
        source: &S,
        host_source: &S2,
    ) -> Result<Vec<(u128, SimItem)>, Box<dyn std::error::Error>>
    where
        S: StateSource,
        S2: StateSource,
    {
        let mut vec = Vec::with_capacity(n);
        let n = self.write_best_valid_to(vec.spare_capacity_mut(), source, host_source)?;
        // SAFETY: We just wrote n items.
        unsafe { vec.set_len(n) };
        Ok(vec)
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
    pub fn get(&self, cache_rank: u128) -> Option<SimItem> {
        self.inner.read().items.get(&cache_rank).cloned()
    }

    /// Remove an item by key.
    pub fn remove(&self, cache_rank: u128) -> Option<SimItem> {
        let mut inner = self.inner.write();
        inner.remove(cache_rank)
    }

    /// Remove an item by key, and prevent it from being re-added for a while.
    pub fn remove_and_disallow(&self, cache_rank: u128) -> Option<SimItem> {
        let mut inner = self.inner.write();
        inner.remove_and_disallow(cache_rank)
    }

    /// Add a bundle to the cache.
    pub fn add_bundle(&self, bundle: SignetEthBundle, basefee: u64) -> Result<(), CacheError> {
        if bundle.replacement_uuid().is_none() {
            // If the bundle does not have a replacement UUID, we cannot add it to the cache.
            return Err(CacheError::BundleWithoutReplacementUuid);
        }

        let item = SimItem::try_from(bundle)?;
        let cache_rank = item.calculate_total_fee(basefee);

        let mut inner = self.inner.write();
        inner.add_inner(cache_rank, item, self.capacity);

        Ok(())
    }

    /// Add an iterator of bundles to the cache. This locks the cache only once
    ///
    /// Bundles added should have a valid replacement UUID. Bundles without a replacement UUID will be skipped.
    pub fn add_bundles<I, Item>(&self, item: I, basefee: u64)
    where
        I: IntoIterator<Item = Item>,
        Item: Into<RecoveredBundle>,
    {
        let mut inner = self.inner.write();
        inner.add_bundles(item, basefee, self.capacity);
    }

    /// Add a transaction to the cache.
    pub fn add_tx(&self, tx: Recovered<TxEnvelope>, basefee: u64) {
        let item = SimItem::from(tx);
        let cache_rank = item.calculate_total_fee(basefee);

        let mut inner = self.inner.write();
        inner.add_inner(cache_rank, item, self.capacity);
    }

    /// Add an iterator of transactions to the cache. This locks the cache only once
    pub fn add_txs<I>(&self, item: I, basefee: u64)
    where
        I: IntoIterator<Item = Recovered<TxEnvelope>>,
    {
        let mut inner = self.inner.write();
        inner.add_txs(item, basefee, self.capacity);
    }

    /// Clean the cache by removing bundles that are not valid in the current
    /// block.
    pub fn clean(&self, block_number: u64, block_timestamp: u64) {
        let mut inner = self.inner.write();
        inner.clean(self.capacity, block_number, block_timestamp);
    }

    /// Clear the cache.
    pub fn clear(&self) {
        let mut inner = self.inner.write();
        inner.clear();
    }
}

/// Internal cache data, meant to be protected by a lock.
struct CacheStore {
    /// Key is the cache_rank, unique ID within the cache && the item's order in the cache. Value is [`SimItem`] itself.
    items: BTreeMap<u128, SimItem>,

    /// Key is the unique identifier for the [`SimItem`] - the UUID for
    /// bundles, tx hash for transactions.
    seen: HashSet<SimIdentifier<'static>>,

    /// Identifiers of items that have been removed from the cache, as
    /// they will never be valid again
    disallowed: LruCache<SimIdentifier<'static>, ()>,
}

impl fmt::Debug for CacheStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheInner").finish()
    }
}

impl CacheStore {
    fn new() -> Self {
        Self {
            items: BTreeMap::new(),
            seen: HashSet::new(),
            disallowed: LruCache::new(NonZeroUsize::new(128).unwrap()),
        }
    }

    /// Add an item to the cache.
    fn add_inner(&mut self, mut cache_rank: u128, item: SimItem, capacity: usize) {
        // Check if we've already seen this item - if so, don't add it
        if !self.seen.insert(item.identifier_owned()) {
            return;
        }

        // If it has the same cache_rank, we decrement (prioritizing earlier items)
        while self.items.contains_key(&cache_rank) && cache_rank != 0 {
            cache_rank = cache_rank.saturating_sub(1);
        }

        if self.items.len() >= capacity {
            // If we are at capacity, we need to remove the lowest score
            if let Some((_, item)) = self.items.pop_first() {
                self.seen.remove(&item.identifier_owned());
            }
        }

        self.items.insert(cache_rank, item.clone());
    }

    fn add_bundles<I, T>(&mut self, item: I, basefee: u64, capacity: usize)
    where
        I: IntoIterator<Item = T>,
        T: Into<RecoveredBundle>,
    {
        for item in item.into_iter() {
            let item = item.into();
            let Ok(item) = SimItem::try_from(item) else {
                // Skip invalid bundles
                continue;
            };
            let cache_rank = item.calculate_total_fee(basefee);
            self.add_inner(cache_rank, item, capacity);
        }
    }

    fn add_txs<I>(&mut self, item: I, basefee: u64, capacity: usize)
    where
        I: IntoIterator<Item = Recovered<TxEnvelope>>,
    {
        for item in item.into_iter() {
            let item = SimItem::from(item);
            let cache_rank = item.calculate_total_fee(basefee);
            self.add_inner(cache_rank, item, capacity);
        }
    }

    /// Remove an item by key. This will also remove it from the seen set.
    fn remove(&mut self, cache_rank: u128) -> Option<SimItem> {
        if let Some(item) = self.items.remove(&cache_rank) {
            self.seen.remove(item.identifier().as_bytes());
            Some(item)
        } else {
            None
        }
    }
    /// Remove an item by key, and prevent it from being re-added for a while.
    /// This will also remove it from the seen set.
    fn remove_and_disallow(&mut self, cache_rank: u128) -> Option<SimItem> {
        self.remove(cache_rank).inspect(|item| {
            self.disallowed.put(item.identifier_owned(), ());
        })
    }

    /// Clean the cache by evicting the lowest-score items and removing bundles
    /// that are not valid in the current block.
    fn clean(&mut self, capacity: usize, block_number: u64, block_timestamp: u64) {
        // Trim to capacity by dropping lower fees.
        while self.items.len() > capacity {
            if let Some(key) = self.items.keys().next() {
                self.remove_and_disallow(*key);
            }
        }

        self.items.retain(|_, item| {
            // Retain only items that are not bundles or are valid in the current block.
            if let SimItem::Bundle(bundle) = item.deref() {
                let ts_range = bundle.valid_timestamp_range();
                let bundle_block = bundle.block_number();

                // NB: we don't need to recheck max_timestamp here, as never
                // covers that.
                let now = block_number == bundle_block && ts_range.contains(&block_timestamp);

                // Never valid if the block number is past the bundle's target
                // block or timestamp is past the bundle's max timestamp
                let never =
                    !now && (block_number > bundle_block || block_timestamp > *ts_range.end());

                if !now {
                    self.seen.remove(item.identifier().as_bytes());
                }

                if never {
                    self.disallowed.put(item.identifier_owned(), ());
                }

                now
            } else {
                true // Non-bundle items are retained
            }
        });
    }

    fn clear(&mut self) {
        self.items.clear();
        self.seen.clear();
    }
}

#[cfg(test)]
mod test {

    use alloy::primitives::{b256, Address};

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

    #[test]
    fn test_cache_with_bundles() {
        let items = vec![
            invalid_bundle_with_score(100, 1, "fbcbb9ce-2bef-4587-9c5f-61f606ca0a1a".to_string()),
            invalid_bundle_with_score(100, 2, "39637ce4-5f33-4eb6-8893-8cc325a6cca3".to_string()),
            invalid_bundle_with_score(100, 3, "1c008717-b187-4e53-9601-25435f5fe8b7".to_string()),
        ];

        let cache = SimCache::with_capacity(2);

        cache.add_bundles(items.clone(), 0);

        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(300), Some(items[2].clone().try_into().unwrap()));
        assert_eq!(cache.get(200), Some(items[1].clone().try_into().unwrap()));
        assert_eq!(cache.get(100), None);
    }

    fn invalid_bundle_with_score(
        gas_limit: u64,
        mpfpg: u128,
        replacement_uuid: String,
    ) -> signet_bundle::RecoveredBundle {
        let tx = invalid_tx_with_score(gas_limit, mpfpg);
        signet_bundle::RecoveredBundle::new(
            vec![tx],
            vec![],
            1,
            Some(2),
            Some(3),
            vec![],
            Some(replacement_uuid.clone()),
            vec![],
            None,
            None,
            vec![],
            Default::default(),
        )
    }

    fn invalid_tx_with_score(
        gas_limit: u64,
        mpfpg: u128,
    ) -> Recovered<alloy::consensus::TxEnvelope> {
        let tx = build_alloy_tx(gas_limit, mpfpg);

        Recovered::new_unchecked(
            TxEnvelope::Eip1559(alloy::consensus::Signed::new_unhashed(
                tx,
                alloy::signers::Signature::test_signature(),
            )),
            Address::with_last_byte(7),
        )
    }

    fn invalid_tx_with_score_and_hash(
        gas_limit: u64,
        mpfpg: u128,
        hash: alloy::primitives::B256,
    ) -> Recovered<alloy::consensus::TxEnvelope> {
        let tx = build_alloy_tx(gas_limit, mpfpg);

        Recovered::new_unchecked(
            TxEnvelope::Eip1559(alloy::consensus::Signed::new_unchecked(
                tx,
                alloy::signers::Signature::test_signature(),
                hash,
            )),
            Address::with_last_byte(8),
        )
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
