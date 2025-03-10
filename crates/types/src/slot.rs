//! Utilities for fetching slots from block timestamps for Signet.
/// A slot calculator, which can calculate the slot for a given timestamp.
#[derive(Debug, Copy, Clone)]
pub struct SlotCalculator {
    /// The start timestamp.
    start_timestamp: u64,
    /// The offset in slots. This is needed for chains that are not PoS right from the start of the network, such as Ethereum Mainnet.
    /// Networks that are PoS from genesis, such as Holesky, have an offset of 0.
    slot_offset: u64,
    /// The slot duration.
    slot_duration: u64,
}

impl SlotCalculator {
    /// Creates a new slot calculator.
    pub const fn new(start_timestamp: u64, slot_offset: u64, slot_duration: u64) -> Self {
        Self {
            start_timestamp,
            slot_offset,
            slot_duration,
        }
    }

    /// Creates a new slot calculator for Holesky.
    pub const fn holesky() -> Self {
        Self {
            start_timestamp: 1695902400,
            slot_offset: 0,
            slot_duration: 12,
        }
    }

    /// Creates a new slot calculator for Ethereum mainnet.
    pub const fn mainnet() -> Self {
        Self {
            start_timestamp: 1663224179,
            slot_offset: 4700013,
            slot_duration: 12,
        }
    }

    /// Calculates the slot for a given timestamp.
    pub const fn calculate_slot(&self, timestamp: u64) -> u64 {
        let elapsed = timestamp - self.start_timestamp;
        let slots = elapsed.saturating_div(self.slot_duration);
        slots + self.slot_offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_slot_calculations() {
        let calculator = SlotCalculator::new(0, 0, 12);
        assert_eq!(calculator.calculate_slot(0), 0);
        assert_eq!(calculator.calculate_slot(12), 1);
        assert_eq!(calculator.calculate_slot(35), 2);
        assert_eq!(calculator.calculate_slot(36), 3);
    }

    #[test]
    fn test_holesky_slot_calculations() {
        let calculator = SlotCalculator::holesky();
        assert_eq!(calculator.calculate_slot(1695902400), 0);
        assert_eq!(calculator.calculate_slot(1695902412), 1);
        assert_eq!(calculator.calculate_slot(1695902416), 1);
        assert_eq!(calculator.calculate_slot(1738866996), 3580383);
    }

    #[test]
    fn test_mainnet_slot_calculations() {
        let calculator = SlotCalculator::mainnet();
        assert_eq!(calculator.calculate_slot(1738863035), 11003251);
        assert_eq!(calculator.calculate_slot(1738866239), 11003518);
        assert_eq!(calculator.calculate_slot(1738866227), 11003517);
    }
}
