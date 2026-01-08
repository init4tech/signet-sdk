use crate::{cache::StateSource, CacheError, SimItemValidity};
use alloy::{
    consensus::{
        transaction::{Recovered, SignerRecoverable},
        Transaction, TxEnvelope,
    },
    primitives::{Address, TxHash, U256},
};
use signet_bundle::{RecoveredBundle, SignetEthBundle, TxRequirement};
use std::{
    borrow::{Borrow, Cow},
    collections::BTreeMap,
    hash::Hash,
    sync::Arc,
};

/// An item that can be simulated, wrapped in an Arc for cheap cloning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimItem {
    /// A bundle to be simulated.
    Bundle(Arc<RecoveredBundle>),
    /// A transaction to be simulated.
    Tx(Arc<Recovered<TxEnvelope>>),
}

impl TryFrom<SignetEthBundle> for SimItem {
    type Error = CacheError;

    fn try_from(bundle: SignetEthBundle) -> Result<Self, Self::Error> {
        bundle.try_into_recovered().map_err(CacheError::BundleRecover).and_then(TryInto::try_into)
    }
}

impl TryFrom<RecoveredBundle> for SimItem {
    type Error = CacheError;

    fn try_from(bundle: RecoveredBundle) -> Result<Self, Self::Error> {
        if bundle.replacement_uuid().is_some() {
            Ok(Self::Bundle(bundle.into()))
        } else {
            Err(CacheError::BundleWithoutReplacementUuid)
        }
    }
}

impl From<Recovered<TxEnvelope>> for SimItem {
    fn from(tx: Recovered<TxEnvelope>) -> Self {
        Self::Tx(tx.into())
    }
}

impl TryFrom<TxEnvelope> for SimItem {
    type Error = CacheError;

    fn try_from(tx: TxEnvelope) -> Result<Self, Self::Error> {
        tx.try_into_recovered().map_err(Into::into).map(Self::from)
    }
}

impl SimItem {
    /// Get the bundle if it is a bundle.
    pub fn as_bundle(&self) -> Option<&RecoveredBundle> {
        match self {
            Self::Bundle(bundle) => Some(bundle),
            Self::Tx(_) => None,
        }
    }

    /// Get the transaction if it is a transaction.
    pub fn as_tx(&self) -> Option<&Recovered<TxEnvelope>> {
        match self {
            Self::Bundle(_) => None,
            Self::Tx(tx) => Some(tx),
        }
    }

    /// Calculate the maximum gas fee payable, this may be used as a heuristic
    /// to determine simulation order.
    pub fn calculate_total_fee(&self, basefee: u64) -> u128 {
        match self {
            Self::Bundle(bundle) => {
                let mut total_tx_fee = 0;
                for tx in bundle.txs() {
                    total_tx_fee += tx.effective_gas_price(Some(basefee)) * tx.gas_limit() as u128;
                }
                total_tx_fee
            }
            Self::Tx(tx) => tx.effective_gas_price(Some(basefee)) * tx.gas_limit() as u128,
        }
    }
}

// Testing functions
impl SimItem {
    /// Returns a unique identifier for this item, which can be used to
    /// distinguish it from other items.
    pub fn identifier(&self) -> SimIdentifier<'_> {
        match self {
            Self::Bundle(bundle) => {
                SimIdentifier::Bundle(Cow::Borrowed(bundle.replacement_uuid().unwrap()))
            }
            Self::Tx(tx) => SimIdentifier::Tx(*tx.inner().hash()),
        }
    }

    /// Returns an unique, owned identifier for this item.
    pub fn identifier_owned(&self) -> SimIdentifier<'static> {
        match self {
            Self::Bundle(bundle) => {
                SimIdentifier::Bundle(Cow::Owned(bundle.replacement_uuid().unwrap().to_string()))
            }
            Self::Tx(tx) => SimIdentifier::Tx(*tx.inner().hash()),
        }
    }

    fn check_tx<S>(&self, source: &S) -> Result<SimItemValidity, Box<dyn std::error::Error>>
    where
        S: StateSource,
    {
        let item = self.as_tx().expect("SimItem is not a Tx");

        let total = item.max_fee_per_gas() * item.gas_limit() as u128;

        source
            .map(&item.signer(), |info| {
                // if the chain nonce is greater than the tx nonce, it is
                // no longer valid
                if info.nonce > item.nonce() {
                    return SimItemValidity::Never;
                }
                // if the chain nonce is less than the tx nonce, we need to wait
                if info.nonce < item.nonce() {
                    return SimItemValidity::Future;
                }
                // if the balance is insufficient, we need to wait
                if info.balance < U256::from(total) {
                    return SimItemValidity::Future;
                }
                // nonce is equal and balance is sufficient
                SimItemValidity::Now
            })
            .map_err(Into::into)
    }

    fn check_bundle_tx_list<S>(
        items: impl Iterator<Item = TxRequirement>,
        source: &S,
    ) -> Result<SimItemValidity, S::Error>
    where
        S: StateSource,
    {
        // For bundles, we want to check the nonce of each transaction. To do
        // this, we build a small in memory cache so that if the same signer
        // appears, we can reuse the nonce info. We do not check balances after
        // the first tx, as they may have changed due to prior txs in the
        // bundle.

        let mut nonce_cache: BTreeMap<Address, u64> = BTreeMap::new();
        let mut items = items.peekable();

        // Peek to perform the balance check for the first tx
        if let Some(first) = items.peek() {
            let info = source.account_details(&first.signer)?;

            // check balance for the first tx is sufficient
            if first.max_fee > info.balance {
                return Ok(SimItemValidity::Future);
            }

            // Cache the nonce. This will be used for the first tx.
            nonce_cache.insert(first.signer, info.nonce);
        }

        for requirement in items {
            let state_nonce = match nonce_cache.get(&requirement.signer) {
                Some(cached_nonce) => *cached_nonce,
                None => {
                    let nonce = source.nonce(&requirement.signer)?;
                    nonce_cache.insert(requirement.signer, nonce);
                    nonce
                }
            };

            if requirement.nonce < state_nonce {
                return Ok(SimItemValidity::Never);
            }
            if requirement.nonce > state_nonce {
                return Ok(SimItemValidity::Future);
            }

            // Increment the cached nonce for the next transaction from this
            // signer. Map _must_ have the entry as we just either loaded or
            // stored it above
            nonce_cache.entry(requirement.signer).and_modify(|n| *n += 1);
        }

        // All transactions passed
        Ok(SimItemValidity::Now)
    }

    fn check_bundle<S, S2>(
        &self,
        source: &S,
        host_source: &S2,
    ) -> Result<SimItemValidity, Box<dyn std::error::Error>>
    where
        S: StateSource,
        S2: StateSource,
    {
        let item = self.as_bundle().expect("SimItem is not a Bundle");

        let ru_tx = Self::check_bundle_tx_list(item.tx_reqs(), source)?;
        let host_tx = Self::check_bundle_tx_list(item.host_tx_reqs(), host_source)?;

        // Check both the regular txs and the host txs.
        Ok(ru_tx.min(host_tx))
    }

    /// Check if the item is valid against the provided state sources.
    ///
    /// This will check that nonces and balances are sufficient for the item to
    /// be included on the current state.
    pub fn check<S, S2>(
        &self,
        source: &S,
        host_source: &S2,
    ) -> Result<SimItemValidity, Box<dyn std::error::Error>>
    where
        S: StateSource,
        S2: StateSource,
    {
        match self {
            SimItem::Bundle(_) => self.check_bundle(source, host_source),
            SimItem::Tx(_) => self.check_tx(source),
        }
    }
}

/// A simulation cache item identifier.
#[derive(Debug, Clone)]
pub enum SimIdentifier<'a> {
    /// A bundle identifier.
    Bundle(Cow<'a, str>),
    /// A transaction identifier.
    Tx(TxHash),
}

impl core::fmt::Display for SimIdentifier<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Bundle(id) => write!(f, "{id}"),
            Self::Tx(id) => write!(f, "{id}"),
        }
    }
}

impl PartialEq for SimIdentifier<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes().eq(other.as_bytes())
    }
}

impl Eq for SimIdentifier<'_> {}

impl Hash for SimIdentifier<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_bytes().hash(state);
    }
}

impl Borrow<[u8]> for SimIdentifier<'_> {
    fn borrow(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl From<TxHash> for SimIdentifier<'_> {
    fn from(tx_hash: TxHash) -> Self {
        Self::Tx(tx_hash)
    }
}

impl SimIdentifier<'_> {
    /// Create a new [`SimIdentifier::Bundle`].
    pub const fn bundle<'a>(id: Cow<'a, str>) -> SimIdentifier<'a> {
        SimIdentifier::Bundle(id)
    }

    /// Create a new [`SimIdentifier::Tx`].
    pub const fn tx(id: TxHash) -> Self {
        Self::Tx(id)
    }

    /// Check if this identifier is a bundle.
    pub const fn is_bundle(&self) -> bool {
        matches!(self, Self::Bundle(_))
    }

    /// Check if this identifier is a transaction.
    pub const fn is_tx(&self) -> bool {
        matches!(self, Self::Tx(_))
    }

    /// Get the identifier as a byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Bundle(id) => id.as_bytes(),
            Self::Tx(id) => id.as_ref(),
        }
    }
}
