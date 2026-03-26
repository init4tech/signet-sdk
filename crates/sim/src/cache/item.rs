use crate::{
    cache::{check_bundle_tx_list, StateSource},
    CacheError, SimItemValidity,
};
use alloy::{
    consensus::{
        transaction::{Recovered, SignerRecoverable},
        Transaction, TxEnvelope,
    },
    primitives::{TxHash, U256},
};
use signet_bundle::{RecoveredBundle, SignetEthBundle, TxRequirement};
use std::{
    borrow::{Borrow, Cow},
    hash::Hash,
    sync::Arc,
};
use tracing::{instrument, trace, trace_span};

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

    async fn check_tx<S>(&self, source: &S) -> Result<SimItemValidity, Box<dyn std::error::Error>>
    where
        S: StateSource,
    {
        let item = self.as_tx().expect("SimItem is not a Tx");
        let signer = item.signer();
        let item_nonce = item.nonce();
        let total = U256::from(item.max_fee_per_gas() * item.gas_limit() as u128) + item.value();

        source
            .map(&signer, |info| {
                let _guard = trace_span!(
                    "check_tx",
                    %signer,
                    item_nonce,
                    expected_nonce = info.nonce,
                )
                .entered();

                // if the chain nonce is greater than the tx nonce, it is
                // no longer valid
                if info.nonce > item_nonce {
                    trace!("nonce too low");
                    return SimItemValidity::Never;
                }
                // if the chain nonce is less than the tx nonce, we need to wait
                if info.nonce < item_nonce {
                    trace!("nonce too high");
                    return SimItemValidity::Future;
                }
                // if the balance is insufficient, we need to wait
                if info.balance < total {
                    trace!(
                        required = %total,
                        available = %info.balance,
                        "insufficient balance",
                    );
                    return SimItemValidity::Future;
                }
                // nonce is equal and balance is sufficient
                SimItemValidity::Now
            })
            .await
            .map_err(Into::into)
    }

    #[instrument(level = "trace", skip_all)]
    async fn check_bundle_tx_list_for_rollup<S>(
        items: impl Iterator<Item = TxRequirement>,
        source: &S,
    ) -> Result<SimItemValidity, S::Error>
    where
        S: StateSource,
    {
        check_bundle_tx_list(items, source).await
    }

    #[instrument(level = "trace", skip_all)]
    async fn check_bundle_tx_list_for_host<S>(
        items: impl Iterator<Item = TxRequirement>,
        source: &S,
    ) -> Result<SimItemValidity, S::Error>
    where
        S: StateSource,
    {
        check_bundle_tx_list(items, source).await
    }

    async fn check_bundle<S, S2>(
        &self,
        source: &S,
        host_source: &S2,
    ) -> Result<SimItemValidity, Box<dyn std::error::Error>>
    where
        S: StateSource,
        S2: StateSource,
    {
        let item = self.as_bundle().expect("SimItem is not a Bundle");

        let ru_tx = Self::check_bundle_tx_list_for_rollup(item.tx_reqs(), source).await?;
        let host_tx = Self::check_bundle_tx_list_for_host(item.host_tx_reqs(), host_source).await?;

        // Check both the regular txs and the host txs.
        Ok(ru_tx.min(host_tx))
    }

    /// Check if the item is valid against the provided state sources.
    ///
    /// This will check that nonces and balances are sufficient for the item to
    /// be included on the current state.
    #[instrument(
        level = "trace",
        name = "preflight_check",
        skip_all,
        fields(
            item_identifier = %self.identifier(),
            item_type = if self.as_bundle().is_some() { "bundle" } else { "tx" },
        ),
        ret(level = "debug", Display),
        err(level = "debug", Display),
    )]
    pub async fn check<S, S2>(
        &self,
        source: &S,
        host_source: &S2,
    ) -> Result<SimItemValidity, Box<dyn std::error::Error>>
    where
        S: StateSource,
        S2: StateSource,
    {
        match self {
            SimItem::Bundle(_) => self.check_bundle(source, host_source).await,
            SimItem::Tx(_) => self.check_tx(source).await,
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
