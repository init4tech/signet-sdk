use signet_types::{AggregateFills, AggregateOrders};
use signet_zenith::RollupOrders;

/// A [`Framed`] containing [`RollupOrders::Order`] instances.
pub type FramedOrders = Framed<RollupOrders::Order>;

/// A [`Framed`] containing [`RollupOrders::Filled`] instances.
pub type FramedFilleds = Framed<RollupOrders::Filled>;

/// Events associated with frame boundaries.
///
/// Events are emitted during EVM execution. These events are emitted within
/// specific callframe boundaries. When a callframe is exited, if it reverted
/// then all events added within that frame must be discarded. Framing the
/// events allows for easy reversion of all events added within a frame.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Framed<T> {
    events: Vec<T>,
    frame_boundaries: Vec<usize>,
}

impl<T> Default for Framed<T> {
    fn default() -> Self {
        Self { events: Default::default(), frame_boundaries: Default::default() }
    }
}

impl<T> Framed<T> {
    /// Make a new `FramedOrders` with the given capacity for orders.
    pub fn with_capacity(capacity: usize) -> Self {
        Self { events: Vec::with_capacity(capacity), frame_boundaries: Vec::new() }
    }

    /// Returns the number of events found, including those that may yet be
    /// reverted.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Returns `true` if the run has no events.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Enter an execution frame.
    pub fn enter_frame(&mut self) {
        self.frame_boundaries.push(self.events.len());
    }

    /// Revert the current frame, discarding all orders added since the last
    /// `enter_frame` call.
    ///
    /// # Panics
    ///
    /// Panics if there are no frames to revert.
    pub fn revert_frame(&mut self) {
        let len = self.frame_boundaries.pop().unwrap();
        self.events.truncate(len);
    }

    /// Exit the current frame.
    ///
    /// # Panics
    ///
    /// Panics if there are no frames to exit.
    pub fn exit_frame(&mut self) {
        self.frame_boundaries.pop().unwrap();
    }

    /// Push an order to the current frame.
    pub fn add(&mut self, order: T) {
        self.events.push(order);
    }

    /// True if all frames have been exited.
    pub fn is_complete(&self) -> bool {
        self.frame_boundaries.is_empty()
    }
}

impl FramedOrders {
    /// Aggregate all orders, producing a single [`AggregateOrders`] instance.
    pub fn aggregate(&self) -> AggregateOrders {
        self.events.iter().collect()
    }
}

impl FramedFilleds {
    /// Aggregate all fills, producing a single [`AggregateFills`] instance. The
    /// chain ID is the ID of the chain that emitted the events.
    pub fn aggregate(&self, chain_id: u64) -> AggregateFills {
        let mut ctx = AggregateFills::default();
        for fill in &self.events {
            ctx.add_fill(chain_id, fill);
        }
        ctx
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn basic_usage() {
        let dummy_order = RollupOrders::Order {
            deadline: Default::default(),
            inputs: Default::default(),
            outputs: Default::default(),
        };

        let mut orders = FramedOrders::default();
        assert!(orders.is_empty());

        // simple revert
        orders.enter_frame();

        orders.add(dummy_order.clone());
        assert_eq!(orders.len(), 1);

        orders.revert_frame();
        assert!(orders.is_empty());

        // multi-frame revert
        orders.enter_frame();
        orders.add(dummy_order.clone());
        assert_eq!(orders.len(), 1);

        orders.enter_frame();
        orders.add(dummy_order.clone());
        orders.add(dummy_order.clone());

        orders.enter_frame();
        orders.add(dummy_order.clone());
        orders.add(dummy_order.clone());
        orders.add(dummy_order.clone());
        assert_eq!(orders.len(), 6);

        orders.exit_frame();
        assert_eq!(orders.len(), 6);

        orders.revert_frame();
        assert_eq!(orders.len(), 1);

        orders.exit_frame();
        assert_eq!(orders.len(), 1);
    }
}
