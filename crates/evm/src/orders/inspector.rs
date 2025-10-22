use crate::{FramedFilleds, FramedOrders};
use alloy::{
    primitives::{Address, Log, U256},
    sol_types::SolEvent,
};
use signet_types::{constants::SignetSystemConstants, AggregateFills, AggregateOrders};
use signet_zenith::RollupOrders;
use trevm::{
    helpers::Ctx,
    revm::{
        interpreter::{
            CallInputs, CallOutcome, CreateInputs, CreateOutcome, Interpreter, InterpreterTypes,
        },
        Database, Inspector,
    },
};

/// Inspector used to detect Signet Orders and inform the builder of the
/// fill requirements.
///
/// This inspector is intended to be used with `trevm`. The EVM driver should
/// - call [`OrderDetector::take_aggregates`] to get the aggregate orders
///   and fills produced by that transaction.
/// - ensure that net fills are sufficient to cover the order inputs via
///   [`AggregateFills::checked_remove_ru_tx_events`].
/// - reject transactions which are not sufficiently filled.
///
/// The [`SignetDriver`] has an example of this in the `check_fills_and_accept`
/// function.
///
/// The `OrderDetector` allows an inner inspector to be used as well. This is
/// useful for tracers and other tools that need to inspect the EVM state.
///
/// [`SignetDriver`]: crate::SignetDriver

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderDetector {
    /// The signet system constants.
    constants: SignetSystemConstants,
    /// Orders detected so far, account for EVM reverts
    orders: FramedOrders,
    /// Fills detected so far, accounting for EVM reverts
    filleds: FramedFilleds,
}

impl OrderDetector {
    /// Create a new [`OrderDetector`] with the given `orders` contract address
    /// and `outputs` mapping.
    pub fn new(constants: SignetSystemConstants) -> OrderDetector {
        OrderDetector { constants, orders: Default::default(), filleds: Default::default() }
    }

    /// Get the address of the orders contract.
    pub const fn contract(&self) -> Address {
        self.constants.rollup().orders()
    }

    /// Get the chain ID.
    pub const fn chain_id(&self) -> u64 {
        self.constants.ru_chain_id()
    }

    /// Take the orders from the inspector, clearing it.
    pub fn take(&mut self) -> (FramedOrders, FramedFilleds) {
        (std::mem::take(&mut self.orders), std::mem::take(&mut self.filleds))
    }

    /// Take the orders from the inspector, clearing it, and convert them to
    /// aggregate orders.
    pub fn take_aggregates(&mut self) -> (AggregateFills, AggregateOrders) {
        let (orders, filleds) = self.take();
        (filleds.aggregate(self.chain_id()), orders.aggregate())
    }

    /// Take the inner inspector and the framed events.
    pub fn into_parts(self) -> (FramedOrders, FramedFilleds) {
        (self.orders, self.filleds)
    }

    /// Get a reference to the framed [`RollupOrders::Order`] events.
    pub const fn orders(&self) -> &FramedOrders {
        &self.orders
    }

    /// Get a reference to the framed [`RollupOrders::Filled`] events.
    pub const fn filleds(&self) -> &FramedFilleds {
        &self.filleds
    }
}

impl<Db, Int> Inspector<Ctx<Db>, Int> for OrderDetector
where
    Db: Database,
    Int: InterpreterTypes,
{
    fn log(&mut self, _interp: &mut Interpreter<Int>, _context: &mut Ctx<Db>, log: Log) {
        // skip if the log is not from the orders contract
        if log.address != self.contract() {
            return;
        }

        if let Ok(Log { data, .. }) = RollupOrders::Order::decode_log(&log) {
            self.orders.add(data);
        } else if let Ok(Log { data, .. }) = RollupOrders::Filled::decode_log(&log) {
            self.filleds.add(data);
        }
    }

    fn call(&mut self, _context: &mut Ctx<Db>, _inputs: &mut CallInputs) -> Option<CallOutcome> {
        self.orders.enter_frame();
        None
    }

    fn call_end(
        &mut self,
        _context: &mut Ctx<Db>,
        _inputs: &CallInputs,
        outcome: &mut CallOutcome,
    ) {
        if outcome.result.is_ok() {
            self.orders.exit_frame();
        } else {
            self.orders.revert_frame();
        }
    }

    fn create(
        &mut self,
        _context: &mut Ctx<Db>,
        _inputs: &mut CreateInputs,
    ) -> Option<CreateOutcome> {
        self.orders.enter_frame();
        None
    }

    fn create_end(
        &mut self,
        _context: &mut Ctx<Db>,
        _inputs: &CreateInputs,
        outcome: &mut CreateOutcome,
    ) {
        if outcome.result.is_ok() {
            self.orders.exit_frame();
        } else {
            self.orders.revert_frame();
        }
    }

    fn selfdestruct(&mut self, _contract: Address, _target: Address, _value: U256) {
        self.orders.exit_frame();
    }
}
