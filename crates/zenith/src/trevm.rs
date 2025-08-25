use crate::Zenith;
use alloy::rlp::BufMut;
use trevm::journal::{JournalDecode, JournalDecodeError, JournalEncode};

const ZENITH_HEADER_BYTES: usize = 32 + 32 + 32 + 20 + 32;

impl JournalEncode for Zenith::BlockHeader {
    fn serialized_size(&self) -> usize {
        ZENITH_HEADER_BYTES
    }

    fn encode(&self, buf: &mut dyn BufMut) {
        let Self { rollupChainId, hostBlockNumber, gasLimit, rewardAddress, blockDataHash } = self;

        rollupChainId.encode(buf);
        hostBlockNumber.encode(buf);
        gasLimit.encode(buf);
        rewardAddress.encode(buf);
        blockDataHash.encode(buf);
    }
}

impl JournalDecode for Zenith::BlockHeader {
    fn decode(buf: &mut &[u8]) -> Result<Self, JournalDecodeError> {
        Ok(Self {
            rollupChainId: JournalDecode::decode(buf)?,
            hostBlockNumber: JournalDecode::decode(buf)?,
            gasLimit: JournalDecode::decode(buf)?,
            rewardAddress: JournalDecode::decode(buf)?,
            blockDataHash: JournalDecode::decode(buf)?,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloy::primitives::{Address, B256, U256};

    fn roundtrip<T: JournalDecode + JournalEncode + PartialEq>(expected: &T) {
        let enc = JournalEncode::encoded(expected);
        assert_eq!(enc.len(), expected.serialized_size(), "{}", core::any::type_name::<T>());
        let dec = T::decode(&mut enc.as_ref()).expect("decoding failed");
        assert_eq!(&dec, expected);
    }

    #[test]
    fn journal() {
        roundtrip(&Zenith::BlockHeader {
            rollupChainId: U256::from(1),
            hostBlockNumber: U256::from(1),
            gasLimit: U256::from(1),
            rewardAddress: Address::repeat_byte(0xa),
            blockDataHash: B256::repeat_byte(0xa),
        });
    }
}
