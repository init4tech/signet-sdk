//! Utilities for fetching slots from block timestamps for Signet.

/// A slot calculator, which can calculate the slot number for a given
/// timestamp.
#[derive(Debug, Copy, Clone)]
pub struct SlotCalculator {
    /// The start timestamp.
    start_timestamp: u64,
    /// The offset in slots. This is needed for chains that are not PoS right
    /// from the start of the network, such as Ethereum Mainnet. Networks that
    /// are PoS from genesis, such as Holesky, have an offset of 0.
    slot_offset: u64,
    /// The slot duration.
    slot_duration: u64,
}

impl SlotCalculator {
    /// Creates a new slot calculator.
    pub const fn new(start_timestamp: u64, slot_offset: u64, slot_duration: u64) -> Self {
        Self { start_timestamp, slot_offset, slot_duration }
    }

    /// Creates a new slot calculator for Holesky.
    pub const fn holesky() -> Self {
        // begin slot calculation for Holesky from block number 1, slot number 2, timestamp 1695902424
        // because of a strange 324 second gap between block 0 and 1 which
        // should have been 27 slots, but which is recorded as 2 slots in chain data
        Self { start_timestamp: 1695902424, slot_offset: 2, slot_duration: 12 }
    }

    /// Creates a new slot calculator for Ethereum mainnet.
    pub const fn mainnet() -> Self {
        Self { start_timestamp: 1663224179, slot_offset: 4700013, slot_duration: 12 }
    }

    /// Creates a new slot calculator for Pecorino host network.
    pub const fn pecorino_host() -> Self {
        Self { start_timestamp: 1740681556, slot_offset: 0, slot_duration: 12 }
    }

    /// Calculates the slot for a given timestamp.
    /// This only works for timestamps that are GEQ to the chain's start_timestamp.
    pub const fn calculate_slot(&self, timestamp: u64) -> u64 {
        let elapsed = timestamp - self.start_timestamp;
        let slots = elapsed.div_ceil(self.slot_duration);
        slots + self.slot_offset
    }

    /// Calculates how many seconds into the block window for a given timestamp.
    pub const fn calculate_timepoint_within_slot(&self, timestamp: u64) -> u64 {
        (timestamp - self.slot_utc_offset()) % self.slot_duration
    }

    /// Calculates the start and end timestamps for a given slot
    pub const fn calculate_slot_window(&self, slot_number: u64) -> (u64, u64) {
        let end_of_slot =
            ((slot_number - self.slot_offset) * self.slot_duration) + self.start_timestamp;
        let start_of_slot = end_of_slot - self.slot_duration;
        (start_of_slot, end_of_slot)
    }

    /// The current slot number.
    pub fn current_slot(&self) -> u64 {
        self.calculate_slot(chrono::Utc::now().timestamp() as u64)
    }

    /// The current number of seconds into the block window.
    pub fn current_timepoint_within_slot(&self) -> u64 {
        self.calculate_timepoint_within_slot(chrono::Utc::now().timestamp() as u64)
    }

    /// The timestamp of the first PoS block in the chain.
    pub const fn start_timestamp(&self) -> u64 {
        self.start_timestamp
    }

    /// The slot number of the first PoS block in the chain.
    pub const fn slot_offset(&self) -> u64 {
        self.slot_offset
    }

    /// The slot duration, usually 12 seconds.
    pub const fn slot_duration(&self) -> u64 {
        self.slot_duration
    }

    /// The offset in seconds between UTC time and slot mining times
    const fn slot_utc_offset(&self) -> u64 {
        self.start_timestamp % self.slot_duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_slot_calculations() {
        let calculator = SlotCalculator::new(0, 0, 12);
        assert_eq!(calculator.calculate_slot(0), 0);

        assert_eq!(calculator.calculate_slot(1), 1);
        assert_eq!(calculator.calculate_slot(11), 1);
        assert_eq!(calculator.calculate_slot(12), 1);

        assert_eq!(calculator.calculate_slot(13), 2);
        assert_eq!(calculator.calculate_slot(23), 2);
        assert_eq!(calculator.calculate_slot(24), 2);

        assert_eq!(calculator.calculate_slot(25), 3);
        assert_eq!(calculator.calculate_slot(35), 3);
        assert_eq!(calculator.calculate_slot(36), 3);
    }

    #[test]
    fn test_holesky_slot_calculations() {
        let calculator = SlotCalculator::holesky();
        // block 1 == slot 2 == timestamp 1695902424
        // timestamp 1695902424 == slot 2
        assert_eq!(calculator.calculate_slot(1695902424), 2);
        // the next second, timestamp 1695902425 == slot 3
        assert_eq!(calculator.calculate_slot(1695902425), 3);

        // block 3557085 == slot 3919127 == timestamp 1742931924
        // timestamp 1742931924 == slot 3919127
        assert_eq!(calculator.calculate_slot(1742931924), 3919127);
        // the next second, timestamp 1742931925 == slot 3919128
        assert_eq!(calculator.calculate_slot(1742931925), 3919128);
    }

    #[test]
    fn test_holesky_slot_timepoint_calculations() {
        let calculator = SlotCalculator::holesky();
        // calculate timepoint in slot
        assert_eq!(calculator.calculate_timepoint_within_slot(1695902424), 0);
        assert_eq!(calculator.calculate_timepoint_within_slot(1695902425), 1);
        assert_eq!(calculator.calculate_timepoint_within_slot(1695902435), 11);
        assert_eq!(calculator.calculate_timepoint_within_slot(1695902436), 0);
    }

    #[test]
    fn test_holesky_slot_window() {
        let calculator = SlotCalculator::holesky();
        // calculate slot window
        assert_eq!(calculator.calculate_slot_window(2), (1695902412, 1695902424));
        assert_eq!(calculator.calculate_slot_window(3), (1695902424, 1695902436));
    }

    #[test]
    fn test_mainnet_slot_calculations() {
        let calculator = SlotCalculator::mainnet();
        assert_eq!(calculator.calculate_slot(1663224179), 4700013);
        assert_eq!(calculator.calculate_slot(1663224180), 4700014);

        assert_eq!(calculator.calculate_slot(1738863035), 11003251);
        assert_eq!(calculator.calculate_slot(1738866239), 11003518);
        assert_eq!(calculator.calculate_slot(1738866227), 11003517);
    }

    #[test]
    fn test_mainnet_slot_timepoint_calculations() {
        let calculator = SlotCalculator::mainnet();
        // calculate timepoint in slot
        assert_eq!(calculator.calculate_timepoint_within_slot(1663224179), 0);
        assert_eq!(calculator.calculate_timepoint_within_slot(1663224180), 1);
        assert_eq!(calculator.calculate_timepoint_within_slot(1663224190), 11);
        assert_eq!(calculator.calculate_timepoint_within_slot(1663224191), 0);
    }

    #[test]
    fn test_ethereum_slot_window() {
        let calculator = SlotCalculator::mainnet();
        // calculate slot window
        assert_eq!(calculator.calculate_slot_window(4700013), (1663224167, 1663224179));
        assert_eq!(calculator.calculate_slot_window(4700014), (1663224179, 1663224191));
    }
}
