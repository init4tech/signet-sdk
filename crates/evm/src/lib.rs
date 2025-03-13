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

//! Signet EVM

mod aliases;
pub use aliases::*;

/// Utilities for converting types to Reth primitives.
pub mod convert;
pub use convert::ToRethPrimitive;

mod driver;
pub use driver::SignetDriver;

mod orders;
pub use orders::{Framed, FramedFilleds, FramedOrders, OrderDetector};

mod result;
pub use result::BlockResult;

use signet_types::config::SignetSystemConstants;
use trevm::{
    revm::{
        inspector_handle_register, inspectors::NoOpInspector, Database, DatabaseCommit, EvmBuilder,
        Inspector,
    },
    TrevmBuilder,
};

pub(crate) const BASE_GAS: usize = 21_000;

/// Type alias for EVMs using a [`StateProviderBox`] as the `DB` type for
/// trevm.
pub type RuRevmState = reth::revm::db::State<
    reth::revm::database::StateProviderDatabase<reth::providers::StateProviderBox>,
>;

/// Create a new EVM with the given database.
pub fn signet_evm<Db: Database + DatabaseCommit>(
    db: Db,
    constants: SignetSystemConstants,
) -> EvmNeedsCfg<'static, Db> {
    EvmBuilder::default()
        .with_db(db)
        .with_external_context(OrderDetector::<NoOpInspector>::new(constants))
        .append_handler_register(inspector_handle_register)
        .build_trevm()
}

/// Create a new EVM with the given database and inspector.
pub fn signet_evm_with_inspector<Db, I>(
    db: Db,
    inner: I,
    constants: SignetSystemConstants,
) -> EvmNeedsCfg<'static, Db, I>
where
    I: Inspector<Db>,
    Db: Database + DatabaseCommit,
{
    let inspector = OrderDetector::new_with_inspector(constants, inner);
    EvmBuilder::default()
        .with_db(db)
        .with_external_context(inspector)
        .append_handler_register(inspector_handle_register)
        .build_trevm()
}

/// Test utilities for the Signet EVM impl.
#[cfg(any(test, feature = "test_utils"))]
pub mod test_utils {
    use alloy::primitives::Address;
    use reth::revm::InMemoryDB;
    use signet_types::config::{HostConfig, PredeployTokens, RollupConfig, SignetSystemConstants};

    /// Test chain id for the host chain.
    pub const TEST_HOST_CHAIN_ID: u64 = 1;
    /// Test chain id for the RU chain.
    pub const TEST_RU_CHAIN_ID: u64 = 15;
    /// Test address for the host zenith.
    pub const HOST_ZENITH_ADDRESS: Address = Address::repeat_byte(0xdf);
    /// Test address for the RU zenith.
    pub const RU_ORDERS_ADDRESS: Address = Address::repeat_byte(0xac);
    /// Test address for the host orders.
    pub const HOST_ORDERS_ADDRESS: Address = Address::repeat_byte(0xdc);

    /// Test address for USDC.
    pub const TEST_USDC: Address = Address::repeat_byte(0x01);

    /// Test address for USDT.
    pub const TEST_USDT: Address = Address::repeat_byte(0x02);

    /// Test address for WBTC.
    pub const TEST_WBTC: Address = Address::repeat_byte(0x03);

    /// Create a new set of Signet system constants for testing.
    pub const fn test_signet_constants() -> SignetSystemConstants {
        let usdc = Address::repeat_byte(0x01);
        let usdt = Address::repeat_byte(0x02);
        let wbtc = Address::repeat_byte(0x03);

        SignetSystemConstants::new(
            HostConfig::new(
                TEST_HOST_CHAIN_ID,
                0,
                HOST_ZENITH_ADDRESS,
                HOST_ORDERS_ADDRESS,
                Address::repeat_byte(1),
                Address::repeat_byte(2),
                PredeployTokens::new(usdc, usdt, wbtc),
            ),
            RollupConfig::new(
                TEST_RU_CHAIN_ID,
                RU_ORDERS_ADDRESS,
                Address::repeat_byte(3),
                Address::repeat_byte(4),
                PredeployTokens::new(usdc, usdt, wbtc),
            ),
        )
    }

    /// Create a new Signet EVM with an in-memory database for testing.
    pub fn test_signet_evm() -> super::EvmNeedsCfg<'static, trevm::revm::db::InMemoryDB> {
        let mut trevm = super::signet_evm(InMemoryDB::default(), test_signet_constants());
        trevm.inner_mut_unchecked().cfg_mut().chain_id = TEST_RU_CHAIN_ID;
        trevm
    }
}
