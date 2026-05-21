//! Reusable [`serde::Deserialize`] helpers that enforce structural
//! invariants on sol-shaped types received at JSON trust boundaries.
//!
//! The [`alloy::sol!`] macro emits permissive [`serde::Deserialize`] impls
//! suitable for ABI-shaped JSON. When the same types are deserialized from
//! untrusted input — for instance, orders posted to a tx-cache feed — the
//! structural invariants the contract enforces on-chain (signature length,
//! non-empty input/output vectors) are not re-checked. These helpers
//! re-impose those invariants via `#[serde(deserialize_with = "…")]`.
//!
//! # Example
//!
//! ```ignore
//! use alloy::primitives::Bytes;
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct ValidatedBlob {
//!     #[serde(deserialize_with = "signet_zenith::serde_helpers::deserialize_signature_bytes")]
//!     signature: Bytes,
//! }
//! ```
use alloy::primitives::Bytes;
use serde::{
    de::{Deserializer, Error as DeError, Unexpected},
    Deserialize,
};

/// Length in bytes of a canonical secp256k1 signature `(r, s, v)`.
const SIGNATURE_BYTES: usize = 65;

/// Deserialize [`Bytes`] and require the value to be exactly 65 bytes.
///
/// Errors via [`serde::de::Error::invalid_length`] if the decoded byte
/// string is any other length.
pub fn deserialize_signature_bytes<'de, D>(deserializer: D) -> Result<Bytes, D::Error>
where
    D: Deserializer<'de>,
{
    let bytes = Bytes::deserialize(deserializer)?;
    if bytes.len() != SIGNATURE_BYTES {
        return Err(D::Error::invalid_length(bytes.len(), &"65 bytes"));
    }
    Ok(bytes)
}

/// Deserialize a [`Vec<T>`] and require it to be non-empty.
///
/// Errors via [`serde::de::Error::invalid_value`] if the decoded vector
/// has length zero.
pub fn deserialize_non_empty_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let v = Vec::<T>::deserialize(deserializer)?;
    if v.is_empty() {
        return Err(D::Error::invalid_value(Unexpected::Seq, &"at least one element"));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct Sig {
        #[serde(deserialize_with = "deserialize_signature_bytes")]
        signature: Bytes,
    }

    #[derive(Debug, Deserialize)]
    struct Vec64 {
        #[serde(deserialize_with = "deserialize_non_empty_vec")]
        items: Vec<u64>,
    }

    #[test]
    fn signature_exact_65_ok() {
        let json = format!(r#"{{"signature":"0x{}"}}"#, "ab".repeat(65));
        let s: Sig = serde_json::from_str(&json).unwrap();
        assert_eq!(s.signature.len(), 65);
    }

    #[test]
    fn signature_short_rejected() {
        let err = serde_json::from_str::<Sig>(r#"{"signature":"0x01"}"#).unwrap_err();
        assert!(err.to_string().contains("65 bytes"), "{err}");
    }

    #[test]
    fn signature_long_rejected() {
        let json = format!(r#"{{"signature":"0x{}"}}"#, "ab".repeat(66));
        let err = serde_json::from_str::<Sig>(&json).unwrap_err();
        assert!(err.to_string().contains("65 bytes"), "{err}");
    }

    #[test]
    fn signature_empty_rejected() {
        let err = serde_json::from_str::<Sig>(r#"{"signature":"0x"}"#).unwrap_err();
        assert!(err.to_string().contains("65 bytes"), "{err}");
    }

    #[test]
    fn non_empty_vec_ok() {
        let v: Vec64 = serde_json::from_str(r#"{"items":[1,2,3]}"#).unwrap();
        assert_eq!(v.items, vec![1, 2, 3]);
    }

    #[test]
    fn non_empty_vec_rejected() {
        let err = serde_json::from_str::<Vec64>(r#"{"items":[]}"#).unwrap_err();
        assert!(err.to_string().contains("at least one element"), "{err}");
    }
}
