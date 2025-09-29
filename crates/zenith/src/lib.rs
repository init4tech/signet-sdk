#![doc = include_str!("../README.md")]
#![warn(
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    unreachable_pub,
    clippy::missing_const_for_fn,
    rustdoc::all
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![deny(unused_must_use, rust_2018_idioms)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod bindings;
pub use bindings::{
    mintCall, BundleHelper, HostOrders, Passage, RollupOrders, RollupPassage, Transactor, Zenith,
};

mod block;
pub use block::{decode_txns, encode_txns, Alloy2718Coder, Coder, ZenithBlock, ZenithTransaction};

mod trevm;

use alloy::{
    network::Network,
    primitives::{address, Address},
    providers::Provider,
};

use alloy_core as _;

/// System address with permission to mint tokens on pre-deploys.
/// "tokenadmin"
pub const MINTER_ADDRESS: Address = address!("00000000000000000000746f6b656e61646d696e");

impl<P, N> HostOrders::HostOrdersInstance<P, N>
where
    P: Provider<N>,
    N: Network,
{
    /// Preflight a signed order to see if the transaction would succeed.
    /// # Warning ⚠️
    /// Take care with the rpc endpoint used for this. SignedFills *must* remain private until they mine.
    pub async fn try_fill(
        &self,
        outputs: Vec<RollupOrders::Output>,
        permit: RollupOrders::Permit2Batch,
    ) -> Result<(), alloy::contract::Error> {
        self.fillPermit2(outputs, permit).call().await.map(drop)
    }
}
