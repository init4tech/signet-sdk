use alloy::{consensus::Header, primitives::B256};
use trevm::journal::{JournalDecode, JournalDecodeError, JournalEncode};

/// Metadata for a block journal. This includes the block header, the host
/// height, and the hash of the previous journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalMeta {
    /// The host height.
    host_height: u64,

    /// The hash of the previous journal.
    prev_journal_hash: B256,

    /// The rollup block header.
    header: Header,
}

impl JournalMeta {
    /// Create a new `JournalMeta`.
    pub const fn new(host_height: u64, prev_journal_hash: B256, header: Header) -> Self {
        Self { host_height, prev_journal_hash, header }
    }

    /// Deconstruct the `JournalMeta` into its parts.
    pub fn into_parts(self) -> (u64, B256, Header) {
        (self.host_height, self.prev_journal_hash, self.header)
    }

    /// Get the host height.
    pub const fn host_height(&self) -> u64 {
        self.host_height
    }

    /// Get the previous journal hash.
    pub const fn prev_journal_hash(&self) -> B256 {
        self.prev_journal_hash
    }

    /// Get the rollup block header.
    pub const fn header(&self) -> &Header {
        &self.header
    }

    /// Get the rollup height.
    pub const fn rollup_height(&self) -> u64 {
        self.header.number
    }
}

impl JournalEncode for JournalMeta {
    fn serialized_size(&self) -> usize {
        8 + 32 + self.header.serialized_size()
    }

    fn encode(&self, buf: &mut dyn alloy::rlp::BufMut) {
        self.host_height.encode(buf);
        self.prev_journal_hash.encode(buf);
        self.header.encode(buf);
    }
}

impl JournalDecode for JournalMeta {
    fn decode(buf: &mut &[u8]) -> Result<Self, JournalDecodeError> {
        let host_height = JournalDecode::decode(buf)?;
        let prev_journal_hash = JournalDecode::decode(buf)?;
        let header = JournalDecode::decode(buf)?;

        Ok(Self { host_height, prev_journal_hash, header })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn roundtrip() {
        let original = JournalMeta {
            host_height: 13871,
            prev_journal_hash: B256::repeat_byte(0x7),
            header: Header::default(),
        };

        let buf = original.encoded();

        let decoded = JournalMeta::decode(&mut &buf[..]).unwrap();
        assert_eq!(original, decoded);
    }
}
