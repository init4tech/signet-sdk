/// The validity status of a simulation item.
///
/// These are ordered from least to most valid. An item that is `Never` valid
/// is always invalid, an item that is `Future` valid may become valid in the
/// future, and an item that is `Now` valid is currently valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SimItemValidity {
    /// The item is invalid and should not be simulated.
    Never,
    /// The item is currently invalid, but may become valid in the future.
    ///
    /// For example, this may be due to nonce gaps.
    Future,
    /// The item is valid and can be simulated.
    Now,
}

impl SimItemValidity {
    /// Returns true if the item is valid now.
    pub const fn is_valid_now(&self) -> bool {
        matches!(self, SimItemValidity::Now)
    }

    /// Returns true if the item is never valid.
    pub const fn is_never_valid(&self) -> bool {
        matches!(self, SimItemValidity::Never)
    }

    /// Returns true if the item may be valid in the future.
    pub const fn is_future_valid(&self) -> bool {
        matches!(self, SimItemValidity::Future)
    }
}
