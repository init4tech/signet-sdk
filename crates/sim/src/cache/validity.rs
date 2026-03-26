use crate::cache::StateSource;
use alloy::primitives::Address;
use core::fmt::{self, Display, Formatter};
use signet_bundle::TxRequirement;
use std::collections::BTreeMap;
use tracing::{trace, trace_span};

/// The validity status of a simulation item.
///
/// These are ordered from least to most valid. An item that is `Never` valid
/// is always invalid, an item that is `Future` valid may become valid in the
/// future, and an item that is `Now` valid is currently valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SimItemValidity {
    /// The item is invalid and should not be simulated.
    Never,
    /// The item is currently invalid, but may become valid in the future.
    ///
    /// For example, this may be due to nonce gaps.
    Future,
    /// The item is valid and can be simulated.
    Now,
}

impl SimItemValidity {
    /// Returns true if the item is valid now.
    pub const fn is_valid_now(&self) -> bool {
        matches!(self, SimItemValidity::Now)
    }

    /// Returns true if the item is never valid.
    pub const fn is_never_valid(&self) -> bool {
        matches!(self, SimItemValidity::Never)
    }

    /// Returns true if the item may be valid in the future.
    pub const fn is_future_valid(&self) -> bool {
        matches!(self, SimItemValidity::Future)
    }
}

impl Display for SimItemValidity {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Never => f.write_str("never"),
            Self::Future => f.write_str("future"),
            Self::Now => f.write_str("now"),
        }
    }
}

/// Check a list of bundle transactions for validity against a state source.
///
/// Validates nonces sequentially, building a per-signer nonce cache so that
/// multiple transactions from the same signer are checked with incrementing
/// nonces. The first transaction's balance is also checked.
pub async fn check_bundle_tx_list<S>(
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
        let info = source.account_details(&first.signer).await?;

        // check balance for the first tx is sufficient
        if first.balance > info.balance {
            trace!(
                required = %first.balance,
                available = %info.balance,
                signer = %first.signer,
                "insufficient balance",
            );
            return Ok(SimItemValidity::Future);
        }

        // Cache the nonce. This will be used for the first tx.
        nonce_cache.insert(first.signer, info.nonce);
    }

    for requirement in items {
        let state_nonce = match nonce_cache.get(&requirement.signer) {
            Some(cached_nonce) => *cached_nonce,
            None => {
                let nonce = source.nonce(&requirement.signer).await?;
                nonce_cache.insert(requirement.signer, nonce);
                nonce
            }
        };

        let _guard = trace_span!(
            "check_bundle_tx",
            signer = %requirement.signer,
            item_nonce = requirement.nonce,
            expected_nonce = state_nonce,
        )
        .entered();

        if requirement.nonce < state_nonce {
            trace!("nonce too low");
            return Ok(SimItemValidity::Never);
        }
        if requirement.nonce > state_nonce {
            trace!("nonce too high");
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
