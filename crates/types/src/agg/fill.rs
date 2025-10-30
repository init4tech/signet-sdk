use crate::AggregateOrders;
use crate::MarketError;
use crate::SignedFill;
use alloy::primitives::{Address, U256};
use serde::{Deserialize, Serialize};
use signet_zenith::RollupOrders;
use std::collections::HashMap;

/// The aggregate fills, to be populated via block extracts. Generally used to
/// hold a **running** total of fills for a given user and asset across a block
/// or set of transactions.
///
/// We use the following terminology:
/// - Add: push outputs from [`RollupOrders::Filled`] into the context.
///   [`Self::add_fill`] is called when the filler transfers assets to the
///   recipient specified in an order.
/// - Remove: pull outputs from [`RollupOrders::Order`] from the context. These
///   are called when an order event is emitted by the rollup orders contract.
///   All `Orders` should be aggregated into a single [`AggregateOrders`] before
///   calling [`Self::checked_remove_aggregate`] or
///   [`Self::unchecked_remove_aggregate`].
///
/// ## Example
///
/// ```
/// # use alloy::primitives::{Address, U256};
/// # use signet_zenith::RollupOrders;
/// # use signet_types::{AggregateFills, AggregateOrders};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let fill = RollupOrders::Filled {
/// #   outputs: vec![],
/// # };
/// # let order = RollupOrders::Order {
/// #   deadline: U256::ZERO,
/// #   inputs: vec![],
/// #   outputs: vec![],
/// # };
/// let mut context = AggregateFills::default();
/// // The first argument is the chain ID of the chain that emitted the event
/// // in this case, Ethereum.
/// context.add_fill(1, &fill);
/// context.checked_remove_order(&order)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregateFills {
    /// Outputs to be transferred to the user. These may be on the rollup or
    /// the host or potentially elsewhere in the future.
    fills: HashMap<(u64, Address), HashMap<Address, U256>>,
}

impl AggregateFills {
    /// Create a new aggregate.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the fill balance a specific asset for a specific user.
    pub fn filled(&self, output_asset: &(u64, Address), recipient: Address) -> U256 {
        self.fills.get(output_asset).and_then(|m| m.get(&recipient)).copied().unwrap_or_default()
    }

    /// Check if the context has enough filled for the asset, recipient, and
    /// amount.
    pub fn check_filled(
        &self,
        output_asset: &(u64, Address),
        recipient: Address,
        amount: U256,
    ) -> Result<(), MarketError> {
        if self.filled(output_asset, recipient) < amount {
            return Err(MarketError::InsufficientBalance {
                chain_id: output_asset.0,
                asset: output_asset.1,
                recipient,
                amount,
            });
        }
        Ok(())
    }

    /// Add an unstructured fill to the context. The `chain_id` is the ID of
    /// of the chain on which the fill occurred.
    pub fn add_raw_fill(
        &mut self,
        chain_id: u64,
        asset: Address,
        recipient: Address,
        amount: U256,
    ) {
        let entry = self.fills.entry((chain_id, asset)).or_default().entry(recipient).or_default();
        *entry = entry.saturating_add(amount);
    }

    /// Add the amount filled to context.
    fn add_fill_output(&mut self, chain_id: u64, output: &RollupOrders::Output) {
        self.add_raw_fill(chain_id, output.token, output.recipient, output.amount)
    }

    /// Ingest a new fill into the aggregate. The chain_id is the ID
    /// of the chain which emitted the event.
    ///
    /// # Note:
    ///
    /// This uses saturating arithmetic to avoid panics. If filling more than
    /// [`U256::MAX`], re-examine life choices and don't do that.
    pub fn add_fill(&mut self, chain_id: u64, fill: &RollupOrders::Filled) {
        fill.outputs.iter().for_each(|o| self.add_fill_output(chain_id, o));
    }

    /// Ingest a [`SignedFill`] into the aggregate. The chain_id is the ID
    /// of the chain which emitted the event.
    ///
    /// # Note:
    ///
    /// This uses saturating arithmetic to avoid panics. If filling more than
    /// [`U256::MAX`], re-examine life choices and don't do that.
    pub fn add_signed_fill(&mut self, chain_id: u64, fill: &SignedFill) {
        fill.outputs.iter().for_each(|o| self.add_fill_output(chain_id, o));
    }

    /// Absorb the fills from another context.
    pub fn absorb(&mut self, other: &Self) {
        for (output_asset, recipients) in other.fills.iter() {
            let context_recipients = self.fills.entry(*output_asset).or_default();
            for (recipient, value) in recipients {
                let filled = context_recipients.entry(*recipient).or_default();
                *filled = filled.saturating_add(*value);
            }
        }
    }

    /// Unabsorb the fills from another context.
    pub fn unchecked_unabsorb(&mut self, other: &Self) -> Result<(), MarketError> {
        for (output_asset, recipients) in other.fills.iter() {
            if let Some(context_recipients) = self.fills.get_mut(output_asset) {
                for (recipient, value) in recipients {
                    if let Some(filled) = context_recipients.get_mut(recipient) {
                        *filled =
                            filled.checked_sub(*value).ok_or(MarketError::InsufficientBalance {
                                chain_id: output_asset.0,
                                asset: output_asset.1,
                                recipient: *recipient,
                                amount: *value,
                            })?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Check that the context can remove the aggregate.
    pub fn check_aggregate(&self, aggregate: &AggregateOrders) -> Result<(), MarketError> {
        for (output_asset, recipients) in aggregate.outputs.iter() {
            if !self.fills.contains_key(output_asset) {
                return Err(MarketError::MissingAsset {
                    chain_id: output_asset.0,
                    asset: output_asset.1,
                });
            };

            for (recipient, value) in recipients {
                self.check_filled(output_asset, *recipient, *value)?;
            }
        }
        Ok(())
    }

    /// Take the aggregate of some orders from the context, without checking
    /// in advance whether the context has sufficient fills to remove the
    /// aggregate. If the context does not have sufficient fills, the context
    /// will be left in a bad state after returning an error.
    pub fn unchecked_remove_aggregate(
        &mut self,
        aggregate: &AggregateOrders,
    ) -> Result<(), MarketError> {
        for (output_asset, recipients) in aggregate.outputs.iter() {
            let context_recipients =
                self.fills.get_mut(output_asset).ok_or(MarketError::MissingAsset {
                    chain_id: output_asset.0,
                    asset: output_asset.1,
                })?;

            for (recipient, amount) in recipients {
                let filled = context_recipients.get_mut(recipient).unwrap();
                *filled = filled.saturating_sub(*amount);
            }
        }

        Ok(())
    }

    /// Remove the aggregate of some orders from the context, checking in
    /// advance that the context has sufficient fills to remove the aggregate.
    pub fn checked_remove_aggregate(
        &mut self,
        aggregate: &AggregateOrders,
    ) -> Result<(), MarketError> {
        self.check_aggregate(aggregate)?;

        for (output_asset, recipients) in aggregate.outputs.iter() {
            let context_recipients =
                self.fills.get_mut(output_asset).expect("checked in check_aggregate");

            for (recipient, amount) in recipients {
                let filled = context_recipients.get_mut(recipient).unwrap();
                *filled = filled.checked_sub(*amount).unwrap();
            }
        }

        Ok(())
    }

    /// Check that the context can take the order.
    pub fn check_order(&self, order: &RollupOrders::Order) -> Result<(), MarketError> {
        self.check_aggregate(&std::iter::once(order).collect())
    }

    /// Take the order from the context, checking in advance that the context
    /// has sufficient fills to remove the order.
    pub fn checked_remove_order(&mut self, order: &RollupOrders::Order) -> Result<(), MarketError> {
        let aggregate = std::iter::once(order).collect();
        self.check_aggregate(&aggregate)?;
        self.unchecked_remove_aggregate(&aggregate)
    }

    /// Take the order from the context, without checking in advance that the
    /// context has sufficient fills to remove the order. If the context does
    /// not have sufficient fills, the context will be left in a bad state
    /// after returning an error.
    pub fn unchecked_remove_order(
        &mut self,
        order: &RollupOrders::Order,
    ) -> Result<(), MarketError> {
        let aggregate = std::iter::once(order).collect();
        self.unchecked_remove_aggregate(&aggregate)
    }

    /// Borrow the current fill mapping.
    pub const fn fills(&self) -> &HashMap<(u64, Address), HashMap<Address, U256>> {
        &self.fills
    }

    /// Mutably borrow the current fill mapping
    pub const fn fills_mut(&mut self) -> &mut HashMap<(u64, Address), HashMap<Address, U256>> {
        &mut self.fills
    }

    /// Check the events emitted by a rollup transaction against the context.
    ///
    /// This will process all fills first, and all orders second.
    pub fn check_ru_tx_events(
        &self,
        fills: &AggregateFills,
        orders: &AggregateOrders,
    ) -> Result<(), MarketError> {
        // Check the aggregate against the combined contexts.
        let combined = CombinedContext { context: self, extra: fills };

        combined.check_aggregate(orders)?;

        Ok(())
    }

    /// Check and remove the events emitted by a rollup transaction. This
    /// function allows atomic ingestion of multiple Fills and Orders. If
    /// the check fails, the aggregate will not be mutated.
    ///
    /// This will process all fills first, and all orders second.
    pub fn checked_remove_ru_tx_events(
        &mut self,
        fills: &AggregateFills,
        orders: &AggregateOrders,
    ) -> Result<(), MarketError> {
        self.check_ru_tx_events(fills, orders)?;
        self.absorb(fills);
        self.unchecked_remove_aggregate(orders)
    }

    /// Check and remove the events emitted by a rollup transaction. This
    /// function allows atomic ingestion of multiple Fills and Orders. **If
    /// the check fails, the aggregate may be mutated.**
    pub fn unchecked_remove_ru_tx_events(
        &mut self,
        fills: &AggregateFills,
        orders: &AggregateOrders,
    ) -> Result<(), MarketError> {
        self.absorb(fills);
        self.unchecked_remove_aggregate(orders)
    }
}

/// A combined context for checking aggregates. This allows us to check with
/// fills, without mutating the context.
struct CombinedContext<'a, 'b> {
    context: &'a AggregateFills,
    extra: &'b AggregateFills,
}

impl CombinedContext<'_, '_> {
    /// Get the combined balance of the context and the extra context.
    fn balance(&self, output_asset: &(u64, Address), recipient: Address) -> U256 {
        self.context.filled(output_asset, recipient) + self.extra.filled(output_asset, recipient)
    }

    /// Check if the combined context has enough filled for the asset,
    /// recipient, and amount.
    fn check_filled(
        &self,
        output_asset: &(u64, Address),
        recipient: Address,
        amount: U256,
    ) -> Result<(), MarketError> {
        if self.balance(output_asset, recipient) < amount {
            return Err(MarketError::InsufficientBalance {
                chain_id: output_asset.0,
                asset: output_asset.1,
                recipient,
                amount,
            });
        }
        Ok(())
    }

    /// Check the aggregate against the combined context.
    fn check_aggregate(&self, aggregate: &AggregateOrders) -> Result<(), MarketError> {
        for (output_asset, recipients) in aggregate.outputs.iter() {
            for (recipient, amount) in recipients {
                self.check_filled(output_asset, *recipient, *amount)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use signet_zenith::RollupOrders::{Filled, Order, Output};

    #[test]
    fn basic_fills() {
        let user_a = Address::with_last_byte(1);
        let user_b = Address::with_last_byte(2);

        let asset_a = Address::with_last_byte(3);
        let asset_b = Address::with_last_byte(4);

        // The orders contain the minimum amount for the fill.
        let a_to_a =
            Output { token: asset_a, amount: U256::from(100), recipient: user_a, chainId: 1 };
        let b_to_b =
            Output { token: asset_b, amount: U256::from(200), recipient: user_b, chainId: 1 };
        let a_to_b =
            Output { token: asset_a, amount: U256::from(300), recipient: user_b, chainId: 1 };

        let fill = Filled { outputs: vec![a_to_a, b_to_b, a_to_b] };

        let order =
            Order { deadline: U256::ZERO, inputs: vec![], outputs: vec![a_to_a, b_to_b, a_to_b] };

        let mut context = AggregateFills::default();
        context.add_fill(1, &fill);

        assert_eq!(context.fills().len(), 2);
        assert_eq!(
            context.fills().get(&(1, asset_a)).unwrap().get(&user_a).unwrap(),
            &U256::from(100)
        );
        assert_eq!(
            context.fills().get(&(1, asset_b)).unwrap().get(&user_b).unwrap(),
            &U256::from(200)
        );
        assert_eq!(
            context.fills().get(&(1, asset_a)).unwrap().get(&user_b).unwrap(),
            &U256::from(300)
        );

        context.checked_remove_order(&order).unwrap();
        assert_eq!(context.fills().len(), 2);
        assert_eq!(
            context.fills().get(&(1, asset_a)).unwrap().get(&user_a).unwrap(),
            &U256::from(0)
        );
        assert_eq!(
            context.fills().get(&(1, asset_b)).unwrap().get(&user_b).unwrap(),
            &U256::from(0)
        );
        assert_eq!(
            context.fills().get(&(1, asset_a)).unwrap().get(&user_b).unwrap(),
            &U256::from(0)
        );
    }

    // Empty removal should work
    #[test]
    fn empty_everything() {
        AggregateFills::default()
            .checked_remove_ru_tx_events(&Default::default(), &Default::default())
            .unwrap();
    }

    #[test]
    fn absorb_unabsorb() {
        let mut context_a = AggregateFills::default();
        let mut context_b = AggregateFills::default();
        let user = Address::with_last_byte(1);
        let asset = Address::with_last_byte(2);
        context_a.add_raw_fill(1, asset, user, U256::from(100));
        context_b.add_raw_fill(1, asset, user, U256::from(200));

        let pre_absorb = context_a.clone();
        context_a.absorb(&context_b);
        assert_eq!(context_a.filled(&(1, asset), user), U256::from(300));
        context_a.unchecked_unabsorb(&context_b).unwrap();
        assert_eq!(context_a, pre_absorb);
    }
}
