use crate::Journal;
use alloy::primitives::B256;
use std::{collections::VecDeque, ops::RangeInclusive};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JournalSetError<'a> {
    /// Cannot ingest the journal because it is at the wrong height.
    #[error("wrong height: actual {actual}, expected {expected}")]
    WrongHeight {
        /// The actual height of the journal.
        actual: u64,

        /// The expected height of the journal.
        expected: u64,

        /// The journal.
        journal: Box<Journal<'a>>,
    },

    /// Cannot ingest the journal because it has the wrong previous hash.
    #[error("wrong prev_hash: current {latest_hash}, new journal expected {in_journal}")]
    WrongPrevHash {
        /// The latest hash of the journal.
        latest_hash: B256,

        /// The hash expected during ingestion.
        in_journal: B256,

        /// The journal.
        journal: Box<Journal<'a>>,
    },

    /// Attempted to append_overwrite a journal that is not in the set's range.
    #[error("not in range: start {start:?}, end {end:?}, height {height}")]
    NotInRange {
        /// The start of the expected range.
        start: Option<u64>,

        /// The end of the expected range.
        end: Option<u64>,

        /// The height of the journal.
        height: u64,

        /// The journal
        journal: Box<Journal<'a>>,
    },
}

impl<'a> JournalSetError<'a> {
    /// Converts the error into a journal, discarding error info.
    pub fn into_journal(self) -> Journal<'a> {
        match self {
            Self::WrongHeight { journal, .. } => *journal,
            Self::WrongPrevHash { journal, .. } => *journal,
            Self::NotInRange { journal, .. } => *journal,
        }
    }
}

/// A set of journals, ordered by height and hash.
#[derive(Debug, Clone, Default)]
pub struct JournalSet<'a> {
    /// The set of journals.
    journals: VecDeque<Journal<'a>>,

    /// The latest height, recorded separately so that if the set is drained,
    /// pushing more journals is still checked for consistency.
    latest_height: Option<u64>,

    /// The latest journal hash.
    latest_hash: Option<B256>,
}

impl<'a> JournalSet<'a> {
    /// Creates a new empty `JournalSet`.
    pub const fn new() -> Self {
        Self { journals: VecDeque::new(), latest_height: None, latest_hash: None }
    }

    /// Creates a new `JournalSet` with the specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self { journals: VecDeque::with_capacity(capacity), latest_height: None, latest_hash: None }
    }

    /// Creates a new `JournalSet` from a single [`Journal`].
    pub fn from_journal(journal: Journal<'a>) -> Self {
        let latest_height = Some(journal.rollup_height());
        let latest_hash = Some(journal.journal_hash());
        let mut journals = VecDeque::new();
        journals.push_back(journal);
        Self { journals, latest_height, latest_hash }
    }

    /// Returns the number of journals in the set.
    pub fn len(&self) -> usize {
        self.journals.len()
    }

    /// True if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.journals.is_empty()
    }

    /// Make a [`JournalSetError::NotInRange`].
    fn not_in_range(&self, journal: Journal<'a>) -> JournalSetError<'a> {
        JournalSetError::NotInRange {
            start: self.earliest_height(),
            end: self.latest_height(),
            height: journal.rollup_height(),
            journal: Box::new(journal),
        }
    }

    /// Make a [`JournalSetError::WrongPrevHash`].
    fn wrong_prev_hash(&self, journal: Journal<'a>) -> JournalSetError<'a> {
        JournalSetError::WrongPrevHash {
            latest_hash: self.latest_hash().expect("condition of use"),
            in_journal: journal.prev_journal_hash(),
            journal: Box::new(journal),
        }
    }

    /// Make a [`JournalSetError::WrongHeight`].
    fn wrong_height(&self, journal: Journal<'a>) -> JournalSetError<'a> {
        JournalSetError::WrongHeight {
            actual: journal.rollup_height(),
            expected: self.latest_height().expect("condition of use") + 1,
            journal: Box::new(journal),
        }
    }

    /// Returns the earliest height of a journal in the set.
    pub fn earliest_height(&self) -> Option<u64> {
        if let Some(journal) = self.journals.front() {
            return Some(journal.rollup_height());
        }
        None
    }

    /// Returns the latest hash of a journal in the set.
    pub const fn latest_hash(&self) -> Option<B256> {
        self.latest_hash
    }

    /// Returns the latest height of a journal in the set.
    pub const fn latest_height(&self) -> Option<u64> {
        self.latest_height
    }

    /// Get the index of the header with the rollup height within the inner
    /// set, None if not present
    fn index_of(&self, rollup_height: u64) -> Option<usize> {
        let start = self.earliest_height()?;
        if rollup_height < start || rollup_height > self.latest_height()? {
            return None;
        }

        Some((rollup_height - start) as usize)
    }

    /// Get the block at that height, if it is within the set.
    pub fn get_by_rollup_height(&self, rollup_height: u64) -> Option<&Journal<'a>> {
        let index = self.index_of(rollup_height)?;
        self.journals.get(index)
    }

    /// Returns the range of heights in the set. If the set is empty, returns
    /// `None`.
    pub fn range(&self) -> Option<RangeInclusive<u64>> {
        let start = self.earliest_height()?;
        let end = self.latest_height()?;

        Some(start..=end)
    }

    /// Check that the journal contains the expected next height.
    fn check_last_height(&self, journal: Journal<'a>) -> Result<Journal<'a>, JournalSetError<'a>> {
        // if we have initialized the last_height, the journal should be
        // exactly that height + 1
        if let Some(latest_height) = self.latest_height() {
            if journal.rollup_height() != latest_height + 1 {
                return Err(self.wrong_height(journal));
            }
        }
        Ok(journal)
    }

    /// Check that the journal contains the expected prev_hash
    fn check_prev_hash(&self, journal: Journal<'a>) -> Result<Journal<'a>, JournalSetError<'a>> {
        // if we have journals, the journal's prev hash should match the last
        // journal's hash
        if let Some(latest_hash) = self.latest_hash() {
            if journal.prev_journal_hash() != latest_hash {
                return Err(self.wrong_prev_hash(journal));
            }
        }
        Ok(journal)
    }

    /// Unwind to the height of the journal.
    ///
    /// ## Condition of use:
    ///
    /// Height of the journal must be in range.
    fn unwind_to(&mut self, journal: &Journal<'a>) {
        let Some(idx) = self.index_of(journal.rollup_height()) else {
            unreachable!("condition of use");
        };

        // truncate to idx + 1, then pop the back
        // e.g. if the idx is 2, we want to keep 3 items.
        //      this puts 2 at the back. then we use `pop_back`
        //      to ensure our latest_height and latest_hash are
        //      updated.
        self.journals.truncate(idx + 1);
        self.pop_back();
    }

    fn append_inner(&mut self, journal: Journal<'a>) {
        self.latest_height = Some(journal.rollup_height());
        self.latest_hash = Some(journal.journal_hash());
        self.journals.push_back(journal);
    }

    /// Push the journal into the set.
    pub fn try_append(&mut self, journal: Journal<'a>) -> Result<(), JournalSetError<'a>> {
        // Check the journal's height
        let journal = self.check_last_height(journal)?;
        let journal = self.check_prev_hash(journal)?;

        self.append_inner(journal);

        Ok(())
    }

    /// Appends the journal to the set, removing any journals that conflict
    /// with it.
    ///
    /// This will only succeed if the journal is within the set's range AND
    /// replacing the journal currently at that height would lead to a
    /// consistent history.
    pub fn append_overwrite(&mut self, journal: Journal<'a>) -> Result<(), JournalSetError<'a>> {
        let Some(j) = self.get_by_rollup_height(journal.rollup_height()) else {
            return Err(self.not_in_range(journal));
        };

        // If the journals are identical, do nothin.
        if j.journal_hash() == journal.journal_hash() {
            return Ok(());
        }

        if j.rollup_height() != journal.rollup_height() {
            return Err(self.wrong_height(journal));
        }

        // If they don't have the same prev hash, return an error.
        if j.prev_journal_hash() != journal.prev_journal_hash() {
            return Err(self.wrong_prev_hash(journal));
        }

        self.unwind_to(&journal);
        self.append_inner(journal);
        Ok(())
    }

    /// Pops the front journal from the set.
    pub fn pop_front(&mut self) -> Option<Journal<'a>> {
        self.journals.pop_front()
    }

    /// Pops the back journal from the set.
    pub fn pop_back(&mut self) -> Option<Journal<'a>> {
        let journal = self.journals.pop_back();

        // This also handles the case where the popped header had a height of
        // zero.
        if let Some(journal) = &journal {
            self.latest_height = Some(journal.rollup_height() - 1);
            self.latest_hash = Some(journal.prev_journal_hash());
        }
        journal
    }
}

impl<'a> IntoIterator for JournalSet<'a> {
    type Item = Journal<'a>;
    type IntoIter = std::collections::vec_deque::IntoIter<Journal<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.journals.into_iter()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{HostJournal, JournalMeta};
    use alloy::{consensus::Header, primitives::Bytes};
    use std::borrow::Cow;
    use trevm::{journal::BundleStateIndex, revm::state::Bytecode};

    fn journal_at_heights(host: u64, rollup: u64, prev_hash: B256) -> Journal<'static> {
        let meta = JournalMeta::new(
            host,
            prev_hash,
            Cow::Owned(Header { number: rollup, ..Default::default() }),
        );
        let host = HostJournal::new(meta, Default::default());

        Journal::V1(host)
    }

    #[test]
    fn basic_consistency() {
        let mut set = JournalSet::new();

        let j0 = journal_at_heights(100, 0, B256::repeat_byte(0));
        let j1 = journal_at_heights(101, 1, j0.journal_hash());
        let j2 = journal_at_heights(102, 2, j1.journal_hash());
        let j3 = journal_at_heights(103, 3, j2.journal_hash());

        // empty set
        assert_eq!(set.earliest_height(), None);
        assert_eq!(set.latest_height(), None);
        assert_eq!(set.latest_hash(), None);
        assert_eq!(set.range(), None);

        // push j0
        assert_eq!(set.try_append(j0.clone()), Ok(()));
        assert_eq!(set.earliest_height(), Some(0));
        assert_eq!(set.latest_height(), Some(0));
        assert_eq!(set.latest_hash(), Some(j0.journal_hash()));
        assert_eq!(set.range(), Some(0..=0));

        // pushing j2 should fail
        assert!(set.try_append(j2.clone()).is_err());

        // push j1
        assert_eq!(set.try_append(j1.clone()), Ok(()));
        assert_eq!(set.earliest_height(), Some(0));
        assert_eq!(set.latest_height(), Some(1));
        assert_eq!(set.latest_hash(), Some(j1.journal_hash()));
        assert_eq!(set.range(), Some(0..=1));

        // pushing j3 should fail
        assert!(set.try_append(j3.clone()).is_err());

        // pop j0 from front
        let popped = set.pop_front().expect("should pop");
        assert_eq!(popped, j0);
        assert_eq!(set.earliest_height(), Some(1));
        assert_eq!(set.latest_height(), Some(1));
        assert_eq!(set.latest_hash(), Some(j1.journal_hash()));

        // push j2
        assert_eq!(set.try_append(j2.clone()), Ok(()));
        assert_eq!(set.earliest_height(), Some(1));
        assert_eq!(set.latest_height(), Some(2));
        assert_eq!(set.latest_hash(), Some(j2.journal_hash()));
        assert_eq!(set.range(), Some(1..=2));

        // push j3
        assert_eq!(set.try_append(j3.clone()), Ok(()));
        assert_eq!(set.earliest_height(), Some(1));
        assert_eq!(set.latest_height(), Some(3));
        assert_eq!(set.latest_hash(), Some(j3.journal_hash()));
        assert_eq!(set.range(), Some(1..=3));

        // pop j1 from front
        let popped = set.pop_front().expect("should pop");
        assert_eq!(popped, j1);
        assert_eq!(set.earliest_height(), Some(2));
        assert_eq!(set.latest_height(), Some(3));
        assert_eq!(set.latest_hash(), Some(j3.journal_hash()));

        // pushing front to back should fail
        assert!(set.try_append(j0.clone()).is_err());
    }

    #[test]
    fn append_overwrite() {
        let mut set = JournalSet::new();

        let j0 = journal_at_heights(100, 0, B256::repeat_byte(0));
        let j1 = journal_at_heights(101, 1, j0.journal_hash());
        let j2 = journal_at_heights(102, 2, j1.journal_hash());
        let j3 = journal_at_heights(103, 3, j2.journal_hash());

        let mut j1_alt_state = BundleStateIndex::default();
        j1_alt_state.new_contracts.insert(
            B256::repeat_byte(1),
            std::borrow::Cow::Owned(Bytecode::new_legacy(Bytes::from_static(&[0, 1, 2, 3]))),
        );
        let j1_alt = Journal::V1(HostJournal::new(
            JournalMeta::new(
                101,
                j0.journal_hash(),
                Cow::Owned(Header { number: 1, ..Default::default() }),
            ),
            j1_alt_state,
        ));

        // push j0-j3
        assert!(set.try_append(j0.clone()).is_ok());
        assert!(set.try_append(j1).is_ok());
        assert!(set.try_append(j2.clone()).is_ok());
        assert!(set.try_append(j3).is_ok());
        assert_eq!(set.len(), 4);

        // overwrite
        assert!(set.append_overwrite(j1_alt).is_ok());
        assert_eq!(set.len(), 2);

        // can't push j2 anymore
        assert!(set.try_append(j2).is_err());
    }
}
