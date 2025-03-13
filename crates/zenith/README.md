## signet-zenith

Rust types & utilities for working with [Zenith](https://github.com/init4tech/zenith) smart contracts.

## What's in this crate?

- [alloy] Bindings for Zenith smart contracts
  - `Zenith`
  - `Passage`
  - `Orders`
- `AggregateOrders` - a struct that holds the net `Order` data for a
  transaction or set of transactions.
- `SignedOrder` - a struct that holds a signed `Order` and the signature
  that was used to sign it. This enables users to make gasless orders using the
  [permit2] orders iunterface.
- `ZenithBlock` - a struct used to decode transaction data from Ethereum blobs
  containing builder-created blocks.

[alloy]: https://docs.rs/alloy/latest/alloy/
[permit2]: https://github.com/Uniswap/permit2
