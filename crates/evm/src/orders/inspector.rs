use crate::{FramedFilleds, FramedOrders};
use alloy::{
    primitives::{map::HashSet, Address, Log, U256},
    sol_types::SolEvent,
};
use signet_types::{constants::SignetSystemConstants, AggregateFills, AggregateOrders};
use signet_zenith::RollupOrders;
use trevm::{
    helpers::Ctx,
    revm::{
        interpreter::{CallInputs, CallOutcome, CreateInputs, CreateOutcome, InterpreterTypes},
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
    contracts: HashSet<Address>,

    /// True if only detecting fills
    fills_only: bool,

    /// The chain ID.
    chain_id: u64,

    /// Orders detected so far, account for EVM reverts
    orders: FramedOrders,

    /// Fills detected so far, accounting for EVM reverts
    filleds: FramedFilleds,
}

impl OrderDetector {
    /// Create a new [`OrderDetector`] with the given `contracts` addresses,
    /// `chain_id`, and `fills_only` flag.
    pub fn new(contracts: HashSet<Address>, chain_id: u64, fills_only: bool) -> Self {
        Self {
            contracts,
            chain_id,
            fills_only,
            orders: Default::default(),
            filleds: Default::default(),
        }
    }

    /// Create a new [`OrderDetector`] for the rollup environment. This detector
    /// will detect both orders and fills.
    pub fn for_rollup(constants: SignetSystemConstants) -> OrderDetector {
        Self::new(
            std::iter::once(constants.rollup().orders()).collect(),
            constants.ru_chain_id(),
            false,
        )
    }

    /// Create a new [`OrderDetector`] for the host environment. This detector
    /// will only detect fills.
    pub fn for_host(constants: SignetSystemConstants) -> OrderDetector {
        Self::new(
            std::iter::once(constants.host().orders()).collect(),
            constants.host_chain_id(),
            true,
        )
    }

    /// Get the address of the orders contract.
    pub fn is_contract(&self, address: Address) -> bool {
        self.contracts.contains(&address)
    }

    /// Get the chain ID.
    pub const fn chain_id(&self) -> u64 {
        self.chain_id
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

    fn enter_frame(&mut self) {
        self.orders.enter_frame();
        self.filleds.enter_frame();
    }

    fn exit_frame(&mut self) {
        self.orders.exit_frame();
        self.filleds.exit_frame();
    }

    fn revert_frame(&mut self) {
        self.orders.revert_frame();
        self.filleds.revert_frame();
    }
}

impl<Db, Int> Inspector<Ctx<Db>, Int> for OrderDetector
where
    Db: Database,
    Int: InterpreterTypes,
{
    fn log(&mut self, _context: &mut Ctx<Db>, log: Log) {
        // skip if the log is not from a configured orders contract
        if !self.is_contract(log.address) {
            return;
        }

        // Try to decode as a filled first
        if let Ok(Log { data, .. }) = RollupOrders::Filled::decode_log(&log) {
            self.filleds.add(data);
            return;
        }

        // Skip any other logs if we're only tracking fills
        if self.fills_only {
            return;
        }

        // Try to decode as an order next
        if let Ok(Log { data, .. }) = RollupOrders::Order::decode_log(&log) {
            if self.fills_only {
                return;
            }
            self.orders.add(data);
        }
    }

    fn call(&mut self, _context: &mut Ctx<Db>, _inputs: &mut CallInputs) -> Option<CallOutcome> {
        self.enter_frame();
        None
    }

    fn call_end(
        &mut self,
        _context: &mut Ctx<Db>,
        _inputs: &CallInputs,
        outcome: &mut CallOutcome,
    ) {
        if outcome.result.is_ok() {
            self.exit_frame();
        } else {
            self.revert_frame();
        }
    }

    fn create(
        &mut self,
        _context: &mut Ctx<Db>,
        _inputs: &mut CreateInputs,
    ) -> Option<CreateOutcome> {
        self.enter_frame();
        None
    }

    fn create_end(
        &mut self,
        _context: &mut Ctx<Db>,
        _inputs: &CreateInputs,
        outcome: &mut CreateOutcome,
    ) {
        if outcome.result.is_ok() {
            self.exit_frame();
        } else {
            self.revert_frame();
        }
    }

    fn selfdestruct(&mut self, _contract: Address, _target: Address, _value: U256) {
        self.exit_frame();
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloy::primitives::Bytes;
    use trevm::{
        helpers::Ctx,
        revm::{
            database::EmptyDB,
            interpreter::{
                interpreter::EthInterpreter, CallInput, CallOutcome, CallScheme, CallValue,
                CreateOutcome, Gas, InstructionResult, InterpreterResult,
            },
            primitives::hardfork::SpecId,
            Context, Inspector,
        },
    };

    type TestCtx = Ctx<EmptyDB>;
    type TestInt = EthInterpreter;

    fn test_ctx() -> TestCtx {
        Context::new(EmptyDB::new(), SpecId::CANCUN)
    }

    fn test_call_inputs() -> CallInputs {
        CallInputs {
            input: CallInput::default(),
            return_memory_offset: 0..0,
            gas_limit: 1_000_000,
            bytecode_address: Address::ZERO,
            known_bytecode: None,
            target_address: Address::ZERO,
            caller: Address::ZERO,
            value: CallValue::Transfer(U256::ZERO),
            scheme: CallScheme::Call,
            is_static: false,
        }
    }

    fn success_outcome() -> CallOutcome {
        CallOutcome::new(
            InterpreterResult::new(InstructionResult::Return, Bytes::new(), Gas::new(1_000_000)),
            0..0,
        )
    }

    fn revert_outcome() -> CallOutcome {
        CallOutcome::new(
            InterpreterResult::new(InstructionResult::Revert, Bytes::new(), Gas::new(1_000_000)),
            0..0,
        )
    }

    fn success_create_outcome() -> CreateOutcome {
        CreateOutcome::new(
            InterpreterResult::new(InstructionResult::Return, Bytes::new(), Gas::new(1_000_000)),
            Some(Address::ZERO),
        )
    }

    fn revert_create_outcome() -> CreateOutcome {
        CreateOutcome::new(
            InterpreterResult::new(InstructionResult::Revert, Bytes::new(), Gas::new(1_000_000)),
            None,
        )
    }

    fn dummy_order() -> RollupOrders::Order {
        RollupOrders::Order {
            deadline: Default::default(),
            inputs: Default::default(),
            outputs: Default::default(),
        }
    }

    fn dummy_filled() -> RollupOrders::Filled {
        RollupOrders::Filled { outputs: Default::default() }
    }

    /// `orders` and `filleds` must be reverted when a call frame reverts.
    #[test]
    fn call_revert_discards_orders_and_filleds() {
        let contract = Address::repeat_byte(0x42);
        let mut detector = OrderDetector::new(std::iter::once(contract).collect(), 1, false);
        let mut ctx = test_ctx();
        let mut inputs = test_call_inputs();

        <OrderDetector as Inspector<TestCtx, TestInt>>::call(&mut detector, &mut ctx, &mut inputs);

        detector.orders.add(dummy_order());
        detector.filleds.add(dummy_filled());
        assert_eq!(detector.orders.len(), 1);
        assert_eq!(detector.filleds.len(), 1);

        let mut outcome = revert_outcome();
        <OrderDetector as Inspector<TestCtx, TestInt>>::call_end(
            &mut detector,
            &mut ctx,
            &inputs,
            &mut outcome,
        );

        assert!(detector.orders.is_empty(), "orders should be empty after call revert");
        assert!(detector.filleds.is_empty(), "filleds should be empty after call revert");
    }

    /// `orders` and `filleds` must be reverted when a create frame reverts.
    #[test]
    fn create_revert_discards_orders_and_filleds() {
        let contract = Address::repeat_byte(0x42);
        let mut detector = OrderDetector::new(std::iter::once(contract).collect(), 1, false);
        let mut ctx = test_ctx();
        let mut inputs = CreateInputs::default();

        <OrderDetector as Inspector<TestCtx, TestInt>>::create(
            &mut detector,
            &mut ctx,
            &mut inputs,
        );

        detector.orders.add(dummy_order());
        detector.filleds.add(dummy_filled());
        assert_eq!(detector.orders.len(), 1);
        assert_eq!(detector.filleds.len(), 1);

        let mut outcome = revert_create_outcome();
        <OrderDetector as Inspector<TestCtx, TestInt>>::create_end(
            &mut detector,
            &mut ctx,
            &inputs,
            &mut outcome,
        );

        assert!(detector.orders.is_empty(), "orders should be empty after create revert");
        assert!(detector.filleds.is_empty(), "filleds should be empty after create revert");
    }

    /// `orders` and `filleds` must be retained on successful call.
    #[test]
    fn call_success_retains_orders_and_filleds() {
        let contract = Address::repeat_byte(0x42);
        let mut detector = OrderDetector::new(std::iter::once(contract).collect(), 1, false);
        let mut ctx = test_ctx();
        let mut inputs = test_call_inputs();

        <OrderDetector as Inspector<TestCtx, TestInt>>::call(&mut detector, &mut ctx, &mut inputs);

        detector.orders.add(dummy_order());
        detector.filleds.add(dummy_filled());
        assert_eq!(detector.orders.len(), 1);
        assert_eq!(detector.filleds.len(), 1);

        let mut outcome = success_outcome();
        <OrderDetector as Inspector<TestCtx, TestInt>>::call_end(
            &mut detector,
            &mut ctx,
            &inputs,
            &mut outcome,
        );

        assert_eq!(detector.orders.len(), 1, "orders should be retained after successful call");
        assert_eq!(detector.filleds.len(), 1, "filleds should be retained after successful call");
    }

    /// `orders` and `filleds` must be retained on successful create.
    #[test]
    fn create_success_retains_orders_and_filleds() {
        let contract = Address::repeat_byte(0x42);
        let mut detector = OrderDetector::new(std::iter::once(contract).collect(), 1, false);
        let mut ctx = test_ctx();
        let mut inputs = CreateInputs::default();

        <OrderDetector as Inspector<TestCtx, TestInt>>::create(
            &mut detector,
            &mut ctx,
            &mut inputs,
        );

        detector.orders.add(dummy_order());
        detector.filleds.add(dummy_filled());
        assert_eq!(detector.orders.len(), 1);
        assert_eq!(detector.filleds.len(), 1);

        let mut outcome = success_create_outcome();
        <OrderDetector as Inspector<TestCtx, TestInt>>::create_end(
            &mut detector,
            &mut ctx,
            &inputs,
            &mut outcome,
        );

        assert_eq!(detector.orders.len(), 1, "orders should be retained after successful create");
        assert_eq!(detector.filleds.len(), 1, "filleds should be retained after successful create");
    }
}
