use crate::JournalMeta;
use alloy::{
    consensus::Header,
    primitives::{keccak256, Bytes, B256},
};
use std::sync::OnceLock;
use trevm::journal::{BundleStateIndex, JournalDecode, JournalDecodeError, JournalEncode};

/// Journal associated with a host block. The journal is an index over the EVM
/// state changes. It can be used to repopulate
#[derive(Debug, Clone)]
pub struct HostJournal<'a> {
    /// The metadata
    meta: JournalMeta<'a>,

    /// The changes.
    journal: BundleStateIndex<'a>,

    /// The serialized journal
    serialized: OnceLock<Bytes>,

    /// The hash of the serialized journal
    hash: OnceLock<B256>,
}

impl PartialEq for HostJournal<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.meta == other.meta && self.journal == other.journal
    }
}

impl Eq for HostJournal<'_> {}

impl<'a> HostJournal<'a> {
    /// Create a new journal.
    pub const fn new(meta: JournalMeta<'a>, journal: BundleStateIndex<'a>) -> Self {
        Self { meta, journal, serialized: OnceLock::new(), hash: OnceLock::new() }
    }

    /// Deconstruct the `HostJournal` into its parts.
    pub fn into_parts(self) -> (JournalMeta<'a>, BundleStateIndex<'a>) {
        (self.meta, self.journal)
    }

    /// Get the journal meta.
    pub const fn meta(&self) -> &JournalMeta<'a> {
        &self.meta
    }

    /// Get the journal.
    pub const fn journal(&self) -> &BundleStateIndex<'a> {
        &self.journal
    }

    /// Get the host height.
    pub const fn host_height(&self) -> u64 {
        self.meta.host_height()
    }

    /// Get the previous journal hash.
    pub const fn prev_journal_hash(&self) -> B256 {
        self.meta.prev_journal_hash()
    }

    /// Get the rollup block header.
    pub fn header(&self) -> &Header {
        self.meta.header()
    }

    /// Get the rollup height.
    pub fn rollup_height(&self) -> u64 {
        self.meta.rollup_height()
    }

    /// Serialize the journal.
    pub fn serialized(&self) -> &Bytes {
        self.serialized.get_or_init(|| JournalEncode::encoded(self))
    }

    /// Serialize and hash the journal.
    pub fn journal_hash(&self) -> B256 {
        *self.hash.get_or_init(|| keccak256(self.serialized()))
    }
}

impl JournalEncode for HostJournal<'_> {
    fn serialized_size(&self) -> usize {
        8 + 32 + self.journal.serialized_size()
    }

    fn encode(&self, buf: &mut dyn alloy::rlp::BufMut) {
        self.meta.encode(buf);
        self.journal.encode(buf);
    }
}

impl JournalDecode for HostJournal<'static> {
    fn decode(buf: &mut &[u8]) -> Result<Self, JournalDecodeError> {
        let original = *buf;

        let meta = JournalMeta::decode(buf)?;
        let journal = JournalDecode::decode(buf)?;

        let bytes_read = original.len() - buf.len();
        let original = &original[..bytes_read];

        Ok(Self {
            meta,
            journal,
            serialized: OnceLock::from(Bytes::copy_from_slice(original)),
            hash: OnceLock::from(keccak256(original)),
        })
    }
}

#[cfg(test)]
pub(crate) mod test {
    use super::*;
    use alloy::primitives::{Address, KECCAK256_EMPTY, U256};
    use std::{borrow::Cow, collections::BTreeMap};
    use trevm::{
        journal::{AcctDiff, InfoOutcome},
        revm::{
            database::states::StorageSlot,
            state::{AccountInfo, Bytecode},
        },
    };

    pub(crate) fn make_state_diff() -> BundleStateIndex<'static> {
        let mut bsi = BundleStateIndex::default();

        let bytecode = Bytecode::new_legacy(Bytes::from_static(b"world"));
        let code_hash = bytecode.hash_slow();

        bsi.new_contracts.insert(code_hash, Cow::Owned(bytecode));

        bsi.state.insert(
            Address::repeat_byte(0x99),
            AcctDiff {
                outcome: InfoOutcome::Diff {
                    old: Cow::Owned(AccountInfo {
                        balance: U256::from(38),
                        nonce: 7,
                        code_hash: KECCAK256_EMPTY,
                        code: None,
                    }),
                    new: Cow::Owned(AccountInfo {
                        balance: U256::from(23828839),
                        nonce: 83,
                        code_hash,
                        code: None,
                    }),
                },
                storage_diff: BTreeMap::from_iter([(
                    U256::MAX,
                    Cow::Owned(StorageSlot {
                        previous_or_original_value: U256::from(123456),
                        present_value: U256::from(654321),
                    }),
                )]),
            },
        );
        bsi
    }

    #[test]
    fn roundtrip() {
        let original = HostJournal::new(
            JournalMeta::new(u64::MAX, B256::repeat_byte(0xff), Cow::Owned(Header::default())),
            make_state_diff(),
        );

        let buf = original.encoded();

        let decoded = HostJournal::decode(&mut &buf[..]).unwrap();
        assert_eq!(original, decoded);
    }
}
