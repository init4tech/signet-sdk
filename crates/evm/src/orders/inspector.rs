use crate::{FramedFilleds, FramedOrders};
use alloy::{
    primitives::{Address, Log, U256},
    sol_types::SolEvent,
};
use signet_types::{config::SignetSystemConstants, MarketContext};
use trevm::revm::{
    inspectors::NoOpInspector,
    interpreter::{
        CallInputs, CallOutcome, CreateInputs, CreateOutcome, EOFCreateInputs, Interpreter,
    },
    Database, EvmContext, Inspector,
};
use zenith_types::RollupOrders;

/// Inspector used to detect Signet Orders and inform the builder of the
/// fill requirements.
///
/// This inspector is intended to be used with `trevm`. The EVM driver should
/// - call [`OrderDetector::take_aggregate`] to get the aggregate orders
///   produced by that transaction.
/// - ensure that net fills are sufficient to cover the order inputs via
///   [`MarketContext::checked_remove_ru_tx_events`].
/// - reject transactions which are not sufficiently filled.
///
/// The [`SignetDriver`] has an example of this in the `check_market_and_accept`
/// function.
///
/// The `OrderDetector` allows an inner inspector to be used as well. This is
/// useful for tracers and other tools that need to inspect the EVM state.
///
/// [`SignetDriver`]: crate::SignetDriver

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderDetector<T = NoOpInspector> {
    /// The signet system constants.
    constants: SignetSystemConstants,
    /// Orders detected so far, account for EVM reverts
    orders: FramedOrders,
    /// Fills detected so far, accounting for EVM reverts
    filleds: FramedFilleds,
    /// The inner inspector (if any)
    inner: T,
}

impl<T> AsRef<T> for OrderDetector<T> {
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

impl<T> AsMut<T> for OrderDetector<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T> OrderDetector<T> {
    /// Create a new [`OrderDetector`] with the given `orders` contract address
    /// and `outputs` mapping.
    pub fn new(constants: SignetSystemConstants) -> OrderDetector<NoOpInspector> {
        OrderDetector {
            constants,
            orders: Default::default(),
            filleds: Default::default(),
            inner: NoOpInspector,
        }
    }

    /// Create a new [`OrderDetector`] with the given `orders` contract address
    /// and an inner inspector.
    pub fn new_with_inspector(constants: SignetSystemConstants, inner: T) -> Self {
        Self { constants, orders: Default::default(), filleds: Default::default(), inner }
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
    pub fn take_aggregate(&mut self) -> (zenith_types::AggregateOrders, MarketContext) {
        let (orders, filleds) = self.take();
        (orders.aggregate(), filleds.aggregate(self.chain_id()))
    }

    /// Take the inner inspector and the framed events.
    pub fn into_parts(self) -> (FramedOrders, FramedFilleds, T) {
        (self.orders, self.filleds, self.inner)
    }

    /// Get a reference to the framed [`RollupOrders::Order`] events.
    pub const fn orders(&self) -> &FramedOrders {
        &self.orders
    }

    /// Get a reference to the framed [`RollupOrders::Filled`] events.
    pub const fn filleds(&self) -> &FramedFilleds {
        &self.filleds
    }

    /// Get a mutable reference to the inner inspector.
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Get a reference to the inner inspector.
    pub const fn inner(&self) -> &T {
        &self.inner
    }
}

impl<Db, T> Inspector<Db> for OrderDetector<T>
where
    Db: Database,
    T: Inspector<Db>,
{
    fn log(&mut self, interp: &mut Interpreter, context: &mut EvmContext<Db>, log: &Log) {
        // skip if the log is not from the orders contract
        if log.address != self.contract() {
            return;
        }

        if let Ok(Log { data, .. }) = RollupOrders::Order::decode_log(log, true) {
            self.orders.add(data);
        } else if let Ok(Log { data, .. }) = RollupOrders::Filled::decode_log(log, true) {
            self.filleds.add(data);
        }

        self.inner.log(interp, context, log)
    }

    fn call(
        &mut self,
        context: &mut EvmContext<Db>,
        inputs: &mut CallInputs,
    ) -> Option<CallOutcome> {
        self.orders.enter_frame();
        self.inner.call(context, inputs)
    }

    fn call_end(
        &mut self,
        context: &mut EvmContext<Db>,
        inputs: &CallInputs,
        outcome: CallOutcome,
    ) -> CallOutcome {
        if outcome.result.is_ok() {
            self.orders.exit_frame();
        } else {
            self.orders.revert_frame();
        }

        self.inner.call_end(context, inputs, outcome)
    }

    fn create(
        &mut self,
        context: &mut EvmContext<Db>,
        inputs: &mut CreateInputs,
    ) -> Option<CreateOutcome> {
        self.orders.enter_frame();
        self.inner.create(context, inputs)
    }

    fn create_end(
        &mut self,
        context: &mut EvmContext<Db>,
        inputs: &CreateInputs,
        outcome: CreateOutcome,
    ) -> CreateOutcome {
        if outcome.result.is_ok() {
            self.orders.exit_frame();
        } else {
            self.orders.revert_frame();
        }
        self.inner.create_end(context, inputs, outcome)
    }

    fn eofcreate(
        &mut self,
        context: &mut EvmContext<Db>,
        inputs: &mut EOFCreateInputs,
    ) -> Option<CreateOutcome> {
        self.orders.enter_frame();
        self.inner.eofcreate(context, inputs)
    }

    fn eofcreate_end(
        &mut self,
        context: &mut EvmContext<Db>,
        inputs: &EOFCreateInputs,
        outcome: CreateOutcome,
    ) -> CreateOutcome {
        if outcome.result.is_ok() {
            self.orders.exit_frame();
        } else {
            self.orders.revert_frame();
        }
        self.inner.eofcreate_end(context, inputs, outcome)
    }

    fn selfdestruct(&mut self, contract: Address, target: Address, value: U256) {
        self.orders.exit_frame();
        self.inner.selfdestruct(contract, target, value)
    }

    fn initialize_interp(&mut self, interp: &mut Interpreter, context: &mut EvmContext<Db>) {
        self.inner.initialize_interp(interp, context)
    }

    fn step(&mut self, interp: &mut Interpreter, context: &mut EvmContext<Db>) {
        self.inner.step(interp, context)
    }

    fn step_end(&mut self, interp: &mut Interpreter, context: &mut EvmContext<Db>) {
        self.inner.step_end(interp, context)
    }
}
