/// A pair of heights, one for the host chain and one for the rollup chain.
///
/// This type helps prevent confusion, rather than just having two `u64` values
/// both named `height`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairedHeights {
    /// The host chain height.
    pub host: u64,
    /// The rollup chain height.
    pub rollup: u64,
}

impl PairedHeights {
    /// Create a new [`PairedHeights`] instance.
    pub const fn new(host: u64, rollup: u64) -> Self {
        Self { host, rollup }
    }
}
