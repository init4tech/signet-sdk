use alloy::primitives::{uint, Address, U160};

/// The address alias offset used for contract senders in magic signatures.
pub(crate) const ADDRESS_ALIAS_OFFSET: U160 =
    uint!(0x1111_0000_0000_0000_0000_0000_0000_0000_0000_1111_U160);

/// Alias an address by adding the address alias offset.
pub fn alias_address(address: Address) -> Address {
    U160::from_be_bytes::<20>(**address).wrapping_add(ADDRESS_ALIAS_OFFSET).into()
}

/// Unalias an address by subtracting the address alias offset.
pub fn unalias_address(address: Address) -> Address {
    U160::from_be_bytes::<20>(**address).wrapping_sub(ADDRESS_ALIAS_OFFSET).into()
}
