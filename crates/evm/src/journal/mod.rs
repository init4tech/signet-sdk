use alloy::primitives::{keccak256, B256};
use std::sync::OnceLock;
use trevm::journal::{BundleStateIndex, JournalDecode, JournalDecodeError, JournalEncode};

/// Journal associated with a host block
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostJournal<'a> {
    /// The host height.
    host_height: u64,

    /// The previous journal hash.
    prev_journal_hash: B256,

    /// The changes.
    journal: BundleStateIndex<'a>,

    /// The serialized journal
    serialized: OnceLock<Vec<u8>>,

    /// The hash of the serialized journal
    hash: OnceLock<B256>,
}

impl<'a> HostJournal<'a> {
    /// Create a new journal.
    pub fn new(host_height: u64, prev_journal_hash: B256, journal: BundleStateIndex<'a>) -> Self {
        Self {
            host_height,
            prev_journal_hash,
            journal,
            serialized: OnceLock::new(),
            hash: OnceLock::new(),
        }
    }

    /// Serialize the journal.
    pub fn serialized(&self) -> &[u8] {
        self.serialized.get_or_init(|| JournalEncode::encoded(self)).as_slice()
    }

    /// Serialize and hash the journal.
    pub fn journal_hash(&self) -> B256 {
        *self.hash.get_or_init(|| keccak256(self.serialized()))
    }
}

impl trevm::journal::JournalEncode for HostJournal<'_> {
    fn serialized_size(&self) -> usize {
        8 + 32 + self.journal.serialized_size()
    }

    fn encode(&self, buf: &mut dyn alloy::rlp::BufMut) {
        self.host_height.encode(buf);
        self.prev_journal_hash.encode(buf);
        self.journal.encode(buf);
    }
}

impl trevm::journal::JournalDecode for HostJournal<'static> {
    fn decode(buf: &mut &[u8]) -> Result<Self, JournalDecodeError> {
        let original = *buf;
        Ok(Self {
            host_height: JournalDecode::decode(buf)?,
            prev_journal_hash: JournalDecode::decode(buf)?,
            journal: JournalDecode::decode(buf)?,
            serialized: OnceLock::from(original.to_vec()),
            hash: OnceLock::from(keccak256(original)),
        })
    }
}
