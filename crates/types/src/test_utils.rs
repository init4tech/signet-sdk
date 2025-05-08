use alloy::{
    primitives::Address,
    signers::{k256::ecdsa::SigningKey, local::PrivateKeySigner},
};
use signet_constants::{HostConfig, PredeployTokens, RollupConstants, SignetSystemConstants};
use std::sync::LazyLock;

pub use signet_constants::test_utils::*;

/// Test signers used in tests.
pub static TEST_SIGNERS: LazyLock<[PrivateKeySigner; 10]> = LazyLock::new(|| {
    [
        PrivateKeySigner::from(SigningKey::from_slice(&[1u8; 32]).unwrap()),
        PrivateKeySigner::from(SigningKey::from_slice(&[2u8; 32]).unwrap()),
        PrivateKeySigner::from(SigningKey::from_slice(&[3u8; 32]).unwrap()),
        PrivateKeySigner::from(SigningKey::from_slice(&[4u8; 32]).unwrap()),
        PrivateKeySigner::from(SigningKey::from_slice(&[5u8; 32]).unwrap()),
        PrivateKeySigner::from(SigningKey::from_slice(&[6u8; 32]).unwrap()),
        PrivateKeySigner::from(SigningKey::from_slice(&[7u8; 32]).unwrap()),
        PrivateKeySigner::from(SigningKey::from_slice(&[8u8; 32]).unwrap()),
        PrivateKeySigner::from(SigningKey::from_slice(&[9u8; 32]).unwrap()),
        PrivateKeySigner::from(SigningKey::from_slice(&[10u8; 32]).unwrap()),
    ]
});

/// Test users used in tests. Addresses corresponding to [`TEST_SIGNERS`].
pub static TEST_USERS: LazyLock<[Address; 10]> =
    LazyLock::new(|| TEST_SIGNERS.each_ref().map(|s| s.address()));
