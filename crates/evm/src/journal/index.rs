use alloy::primitives::{Address, B256, U256};
use reth::providers::ExecutionOutcome;
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
};
use trevm::revm::{
    db::{states::StorageSlot, AccountStatus, BundleAccount, BundleState},
    primitives::{AccountInfo, Bytecode},
};

/// Outcome of an account info after block execution. Post-6780, accounts
/// cannot be destroyed, only created or modified. In either case, the new and
/// old states are contained in this object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InfoOutcome<'a> {
    /// Account was created after block execution.
    ///
    /// Reverting this means deleting the account.
    Created(Cow<'a, AccountInfo>),
    /// Account was modified after block execution. This object contains the
    /// new and previous states.
    ///
    /// Reverting this means restoring the previous state.
    Diff {
        /// The original account info before block execution.
        old: Cow<'a, AccountInfo>,
        /// The updated account info after block execution.
        new: Cow<'a, AccountInfo>,
    },
    /// Account was destroyed after block execution. Restoring this state means
    /// restoring the account.
    Destroyed(Cow<'a, AccountInfo>),
}

impl InfoOutcome<'_> {
    /// Get the original account info. This is `None` if the account was
    /// created.
    pub fn original(&self) -> Option<Cow<'_, AccountInfo>> {
        match self {
            Self::Created(_) => None,
            Self::Diff { old, .. } => Some(Cow::Borrowed(old)),
            Self::Destroyed(info) => Some(Cow::Borrowed(info)),
        }
    }

    /// Get the updated account info. This is the account info at the end of
    /// block execution.
    pub fn updated(&self) -> Cow<'_, AccountInfo> {
        match self {
            Self::Created(info) => Cow::Borrowed(info),
            Self::Diff { new, .. } => Cow::Borrowed(new),
            Self::Destroyed(_) => Cow::Owned(Default::default()),
        }
    }
}

impl<'a> From<&'a BundleAccount> for InfoOutcome<'a> {
    fn from(value: &'a BundleAccount) -> Self {
        match (&value.original_info, &value.info) {
            (None, Some(new)) => Self::Created(Cow::Borrowed(new)),
            (Some(old), Some(new)) => {
                Self::Diff { old: Cow::Borrowed(old), new: Cow::Borrowed(new) }
            }
            (Some(old), None) => {
                Self::Destroyed(Cow::Borrowed(old))
            }
            _ => unreachable!("revm will never output a bundle account that went from not-existing to not-existing"),
        }
    }
}

/// Contains the diff of an account after block execution. This includes the
/// account info and the storage diff. This type ensures that the storage
/// updates are sorted by slot.
///
/// Reverting this means:
/// - Write the original value for the account info (deleting the account if it
///   was created)
/// - Write the original value for each storage slot
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcctDiff<'a> {
    /// Outcome of an account info after block execution.
    pub outcome: InfoOutcome<'a>,
    /// The storage diff for the account. This is a map of storage slot to the
    /// old and new values
    pub storage_diff: BTreeMap<U256, Cow<'a, StorageSlot>>,
}

impl AcctDiff<'_> {
    /// Get the original account info. This is `None` if the account was
    /// created.
    pub fn original(&self) -> Option<Cow<'_, AccountInfo>> {
        self.outcome.original()
    }

    /// Get the updated account info. This is the account info at the end of
    /// block execution.
    pub fn updated(&self) -> Cow<'_, AccountInfo> {
        self.outcome.updated()
    }
}

impl<'a> From<&'a BundleAccount> for AcctDiff<'a> {
    fn from(value: &'a BundleAccount) -> Self {
        let outcome = InfoOutcome::from(value);
        let storage_diff = value
            .storage
            .iter()
            .filter(|(_, v)| v.is_changed())
            .map(|(k, v)| (*k, Cow::Borrowed(v)))
            .collect();
        AcctDiff { outcome, storage_diff }
    }
}

impl From<AcctDiff<'_>> for BundleAccount {
    fn from(value: AcctDiff<'_>) -> Self {
        let original_info = value.outcome.original().map(|info| info.into_owned());
        let info = Some(value.outcome.updated().into_owned());
        let storage = value.storage_diff.into_iter().map(|(k, v)| (k, v.into_owned())).collect();

        BundleAccount { original_info, info, storage, status: AccountStatus::Changed }
    }
}

/// A state index contains the diffs for a single block. The primary purpose of
/// this type is to iterate over the information in a [`BundleState`], making a
/// [`BTreeMap`] containing the changed addresses. This ensures that the
/// state updates are sorted by address, and the bytecodes are sorted by
/// contract address.
///
/// Reverting this type means reverting
/// - Reverting each account state
/// - Deleting each new contract
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BundleStateIndex<'a> {
    /// The state index contains the account and storage diffs for a single
    /// block.
    pub state_index: BTreeMap<Address, AcctDiff<'a>>,
    /// The new contracts created in this block.
    pub new_contracts: BTreeMap<B256, Cow<'a, Bytecode>>,
}

impl<'a> From<&'a ExecutionOutcome> for BundleStateIndex<'a> {
    fn from(value: &'a ExecutionOutcome) -> Self {
        Self::from(&value.bundle)
    }
}

impl<'a> From<&'a BundleState> for BundleStateIndex<'a> {
    fn from(value: &'a BundleState) -> Self {
        let state_index = value
            .state
            .iter()
            .map(|(address, account)| (*address, AcctDiff::from(account)))
            .collect();

        let new_contracts = value.contracts.iter().map(|(k, v)| (*k, Cow::Borrowed(v))).collect();
        BundleStateIndex { state_index, new_contracts }
    }
}

impl From<BundleStateIndex<'_>> for BundleState {
    // much of this implementation adapted from revm:
    // revm/src/db/states/bundle_state.rs
    fn from(value: BundleStateIndex<'_>) -> Self {
        let mut state_size = 0;
        let state: HashMap<_, _> = value
            .state_index
            .into_iter()
            .map(|(address, info)| {
                let original = info.original().map(Cow::into_owned);
                let present = Some(info.updated().into_owned());

                let storage =
                    info.storage_diff.into_iter().map(|(k, v)| (k, v.into_owned())).collect();

                let account: BundleAccount =
                    BundleAccount::new(original, present, storage, AccountStatus::Changed);

                state_size += account.size_hint();
                (address, account)
            })
            .collect();

        let contracts = value.new_contracts.into_iter().map(|(a, c)| (a, c.into_owned())).collect();

        BundleState { state, reverts: Default::default(), contracts, state_size, reverts_size: 0 }
    }
}
