use alloy::{consensus::Header, primitives::B256};
use trevm::journal::{JournalDecode, JournalEncode};

use crate::HostJournal;

/// Journal versions.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Journal<'a> {
    /// Version 1
    V1(HostJournal<'a>),
}

impl<'a> Journal<'a> {
    /// Get the host height.
    pub const fn host_height(&self) -> u64 {
        match self {
            Journal::V1(journal) => journal.host_height(),
        }
    }

    /// Get the previous journal hash.
    pub const fn prev_journal_hash(&self) -> B256 {
        match self {
            Journal::V1(journal) => journal.prev_journal_hash(),
        }
    }

    /// Get the rollup block header.
    pub fn header(&self) -> &Header {
        match self {
            Journal::V1(journal) => journal.header(),
        }
    }

    /// Get a reference to the host journal.
    pub const fn journal(&self) -> &HostJournal<'a> {
        match self {
            Journal::V1(journal) => journal,
        }
    }

    /// Get the journal hash.
    pub fn journal_hash(&self) -> B256 {
        match self {
            Journal::V1(journal) => journal.journal_hash(),
        }
    }

    /// Get the rollup height.
    pub fn rollup_height(&self) -> u64 {
        match self {
            Journal::V1(journal) => journal.rollup_height(),
        }
    }
}

impl JournalEncode for Journal<'_> {
    fn serialized_size(&self) -> usize {
        // 1 byte for the version
        1 + match self {
            Journal::V1(journal) => journal.serialized_size(),
        }
    }

    fn encode(&self, buf: &mut dyn alloy::rlp::BufMut) {
        match self {
            Journal::V1(journal) => {
                1u8.encode(buf);
                journal.encode(buf)
            }
        }
    }
}

impl JournalDecode for Journal<'static> {
    fn decode(buf: &mut &[u8]) -> Result<Self, trevm::journal::JournalDecodeError> {
        let version: u8 = JournalDecode::decode(buf)?;
        match version {
            1 => JournalDecode::decode(buf).map(Journal::V1),
            _ => Err(trevm::journal::JournalDecodeError::InvalidTag {
                ty_name: "Journal",
                tag: version,
                max_expected: 1,
            }),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{host::test::make_state_diff, JournalMeta};
    use std::borrow::Cow;

    #[test]
    fn roundtrip() {
        let journal = Journal::V1(HostJournal::new(
            JournalMeta::new(42, B256::repeat_byte(0x17), Cow::Owned(Header::default())),
            make_state_diff(),
        ));
        let mut buf = Vec::new();
        journal.encode(&mut buf);
        let decoded = Journal::decode(&mut &buf[..]).unwrap();
        assert_eq!(journal, decoded);
    }
}
