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

- **signet-zenith** - [Zenith] contract bindings and related types.
- **signet-types** - Common types and utilities for Signet.
- **signet-extract** - Extracts Signet blocks from an Ethereum block. Also
  includes test utilities for specifying host and rollup blocks.
- **signet-evm** - A wrapper around [trevm] that implements a Signet-specific
  revm inspector for detecting orders, and Signet's block-execution logic.
- **signet-bundle** - Types and utilities for simulating bundles of Signet
  transactions, and determining what fills would be required to include them.
- **signet-test-utils** - Utilities for testing Signet libraries and
  applications.

### Contributing to the SDK

Please see [CONTRIBUTING.md](CONTRIBUTING.md).

### Note on Stability

For most users, we recommend depending on `main` for the most up-to-date
version of the SDK. You can do this by adding lines like the following to your
`Cargo.toml`:

```toml
signet-types = { git = "https://github.com/init4tech/signet-sdk/" branch = "main"}
```

We intend to publish these crates, however, due to dependencies on unpublished
reth crates, we are currently unable to do so. We will be tagging release
versions and adhering to semver as well as possible. However, our dependence on
unstable reth APIs may result in regular breaking changes that do not adhere to
semver. We will do our best to minimize these changes. Reth also suffers
semi-frequent dependency rot, and as a result we cannot guarantee that
any given tagged versions of this crate will build without modification.

[Signet]: https://signet.sh
[trevm]: https://docs.rs/trevm/latest/trevm/
[Signet docs]: https://docs.signet.sh
[Zenith]: https://github.com/init4tech/zenith
