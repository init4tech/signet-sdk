mod error;
pub use error::MarketError;

mod fill;
pub use fill::AggregateFills;

mod order;
pub use order::AggregateOrders;

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::{
        primitives::{address, U256},
        uint,
    };
    use signet_constants::test_utils::HOST_CHAIN_ID;
    use signet_zenith::{HostOrders, RollupOrders};

    #[test]
    fn a_funny_order() {
        // Test the funny order
        let fill = RollupOrders::Filled {
            outputs: vec![HostOrders::Output {
                token: address!("0x885F8DB528dC8a38aA3DDad9D3F619746B4a6A81"),
                amount: U256::from(1_000_000),
                recipient: address!("0x492e9c316f073fE4dE9d665221568cDAD1A7E95b"),
                chainId: HOST_CHAIN_ID as u32,
            }],
        };

        let order = RollupOrders::Order {
            deadline: uint!(0x686fe15a_U256),
            inputs: vec![HostOrders::Input {
                token: address!("0x0b8bc5e60ee10957e0d1a0d95598fa63e65605e2"),
                amount: U256::from(0xf4240),
            }],
            outputs: vec![HostOrders::Output {
                token: address!("0x885F8DB528dC8a38aA3DDad9D3F619746B4a6A81"),
                amount: U256::from(1_000_000),
                recipient: address!("0x492e9c316f073fE4dE9d665221568cDAD1A7E95b"),
                chainId: HOST_CHAIN_ID as u32,
            }],
        };

        let mut fills = AggregateFills::default();
        fills.add_fill(HOST_CHAIN_ID, &fill);

        // fills.checked_remove_order(&order).unwrap();
        let mut agg_orders = AggregateOrders::default();
        agg_orders.ingest(&order);

        fills.checked_remove_ru_tx_events(&agg_orders, &Default::default()).unwrap();
    }
}
