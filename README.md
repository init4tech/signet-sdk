# Signet SDK

A collection of libraries and tools implementing core logic for
[Signet].

### What's new in Signet?

Signet is a pragmatic Ethereum rollup that offers a new set of ideas and aims
to radically modernize rollup technology.

- No proving systems or state roots, drastically reducing computational
  overhead.
- Market-based cross-chain transfers for instant asset movement.
- A controlled block inclusion mechanism to combat block construction
  centralization.
- Conditional transactions for secure, atomic cross-chain operations.

Signet extends the EVM, and is compatible with all existing Ethereum tooling.
Using Signet does not require smart contract modifications, or Signet-specific
knowledge. Signet does not have a native token.

Signet is just a rollup.

See the [Signet docs] for more info.

### What's in the SDK?

- **signet-constants** - Constants for Signet, including chain IDs, contract
  addresses, and other on-chain configuration.
- **signet-zenith** - [Zenith] contract bindings and related types.
- **signet-types** - Common types and utilities for Signet.
- **signet-extract** - Extracts Signet blocks from an Ethereum block. Also
  includes test utilities for specifying host and rollup blocks.
- **signet-evm** - A wrapper around [trevm] that implements a Signet-specific
  revm inspector for detecting orders, and Signet's block-execution logic.
- **signet-journal** - A serializablable, unwindable journal of EVM state
  changes.
- **signet-bundle** - Types and utilities for simulating bundles of Signet
  transactions, and determining what fills would be required to include them.
- **signet-sim** - Block construction library for Signet. Produces blocks from
  a transaction cache by scoring them according to the increase in the
  builder's balance.
- **signet-tx-cache** - A client for Signets tx-cache webservice.
- **signet-test-utils** - Utilities for testing Signet libraries and
  applications.

### Contributing to the SDK

Please see [CONTRIBUTING.md](CONTRIBUTING.md).

[Signet]: https://signet.sh
[trevm]: https://docs.rs/trevm/latest/trevm/
[Signet docs]: https://signet.sh/docs
[Zenith]: https://github.com/init4tech/zenith
