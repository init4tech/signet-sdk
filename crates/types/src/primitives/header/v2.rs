//! [`SignetHeaderV2`] — validated signet header with computed roots.

use super::{check_roots_non_empty, check_shared_defaults, SignetHeaderError};
use alloy::{
    consensus::{BlockHeader, Header},
    primitives::{Address, BlockNumber, Bloom, Bytes, Sealed, B256, B64, U256},
};
use std::ops::Deref;

/// A validated signet header (V2) wrapping a [`Sealed<Header>`].
///
/// V2 headers have all shared fields at their defaults but both
/// `transactions_root` and `receipts_root` set to non-empty values
/// (i.e. not [`EMPTY_ROOT_HASH`]).
/// The block hash is eagerly cached on construction.
///
/// **Unstable** — this type is experimental and not yet used in production.
///
/// [`EMPTY_ROOT_HASH`]: alloy::consensus::constants::EMPTY_ROOT_HASH
#[deprecated(note = "SignetHeaderV2 is unstable and not yet used in production")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignetHeaderV2(Sealed<Header>);

#[allow(deprecated)]
impl SignetHeaderV2 {
    /// Consume the wrapper, returning the inner [`Sealed<Header>`].
    pub fn into_inner(self) -> Sealed<Header> {
        self.0
    }

    /// Get the cached block hash.
    pub const fn hash(&self) -> B256 {
        self.0.hash()
    }
}

#[allow(deprecated)]
impl TryFrom<Header> for SignetHeaderV2 {
    type Error = SignetHeaderError;

    fn try_from(header: Header) -> Result<Self, Self::Error> {
        let must_be_default = check_shared_defaults(&header);
        let must_not_be_default = check_roots_non_empty(&header);

        if must_be_default.is_empty() && must_not_be_default.is_empty() {
            Ok(Self(Sealed::new(header)))
        } else {
            Err(SignetHeaderError { must_be_default, must_not_be_default })
        }
    }
}

#[allow(deprecated)]
impl Deref for SignetHeaderV2 {
    type Target = Sealed<Header>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[allow(deprecated)]
impl AsRef<Header> for SignetHeaderV2 {
    fn as_ref(&self) -> &Header {
        &self.0
    }
}

#[allow(deprecated)]
impl BlockHeader for SignetHeaderV2 {
    fn parent_hash(&self) -> B256 {
        self.0.parent_hash()
    }
    fn ommers_hash(&self) -> B256 {
        self.0.ommers_hash()
    }
    fn beneficiary(&self) -> Address {
        self.0.beneficiary()
    }
    fn state_root(&self) -> B256 {
        self.0.state_root()
    }
    fn transactions_root(&self) -> B256 {
        self.0.transactions_root()
    }
    fn receipts_root(&self) -> B256 {
        self.0.receipts_root()
    }
    fn withdrawals_root(&self) -> Option<B256> {
        self.0.withdrawals_root()
    }
    fn logs_bloom(&self) -> Bloom {
        self.0.logs_bloom()
    }
    fn difficulty(&self) -> U256 {
        self.0.difficulty()
    }
    fn number(&self) -> BlockNumber {
        self.0.number()
    }
    fn gas_limit(&self) -> u64 {
        self.0.gas_limit()
    }
    fn gas_used(&self) -> u64 {
        self.0.gas_used()
    }
    fn timestamp(&self) -> u64 {
        self.0.timestamp()
    }
    fn mix_hash(&self) -> Option<B256> {
        self.0.mix_hash()
    }
    fn nonce(&self) -> Option<B64> {
        self.0.nonce()
    }
    fn base_fee_per_gas(&self) -> Option<u64> {
        self.0.base_fee_per_gas()
    }
    fn blob_gas_used(&self) -> Option<u64> {
        self.0.blob_gas_used()
    }
    fn excess_blob_gas(&self) -> Option<u64> {
        self.0.excess_blob_gas()
    }
    fn parent_beacon_block_root(&self) -> Option<B256> {
        self.0.parent_beacon_block_root()
    }
    fn requests_hash(&self) -> Option<B256> {
        self.0.requests_hash()
    }
    fn extra_data(&self) -> &Bytes {
        self.0.extra_data()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_v2_header() -> Header {
        Header {
            transactions_root: B256::repeat_byte(0x01),
            receipts_root: B256::repeat_byte(0x02),
            ..Default::default()
        }
    }

    #[allow(deprecated)]
    #[test]
    fn v2_accepts_valid_rooted_header() {
        SignetHeaderV2::try_from(valid_v2_header()).unwrap();
    }

    #[allow(deprecated)]
    #[test]
    fn v2_rejects_empty_root_transactions_root() {
        let header = Header { receipts_root: B256::repeat_byte(0x01), ..Default::default() };
        let err = SignetHeaderV2::try_from(header).unwrap_err();
        assert!(err.must_not_be_default.contains(&"transactions_root"));
    }

    #[allow(deprecated)]
    #[test]
    fn v2_rejects_empty_root_receipts_root() {
        let header = Header { transactions_root: B256::repeat_byte(0x01), ..Default::default() };
        let err = SignetHeaderV2::try_from(header).unwrap_err();
        assert!(err.must_not_be_default.contains(&"receipts_root"));
    }

    #[allow(deprecated)]
    #[test]
    fn v2_rejects_non_default_shared_field() {
        let header = Header {
            transactions_root: B256::repeat_byte(0x01),
            receipts_root: B256::repeat_byte(0x02),
            state_root: B256::repeat_byte(0x03),
            ..Default::default()
        };
        let err = SignetHeaderV2::try_from(header).unwrap_err();
        assert!(err.must_be_default.contains(&"state_root"));
    }

    #[allow(deprecated)]
    #[test]
    fn v2_reports_both_violation_kinds() {
        let header = Header { state_root: B256::repeat_byte(0x01), ..Default::default() };
        let err = SignetHeaderV2::try_from(header).unwrap_err();
        assert!(!err.must_be_default.is_empty());
        assert!(!err.must_not_be_default.is_empty());
    }

    #[allow(deprecated)]
    #[test]
    fn v2_into_inner_returns_sealed() {
        let v2 = SignetHeaderV2::try_from(valid_v2_header()).unwrap();
        let sealed = v2.into_inner();
        let _ = sealed.hash();
    }
}
