# Signet SDK

A collection of libraries and tools implementing core logic for
[Signet].

Signet is a pragmatic Ethereum rollup. See the [Signet docs] for more info.

## Libraries

- signet-types - Common types and utilities for Signet.
- signet-extract - Extracts Signet blocks from an Ethereum block.
- signet-evm - A wrapper around [trevm] that implements a Signet-specific order
  detector and block-execution logic.
- signet-bundle - Flashbots-like bundle types and bundle simulation logic.
- signet-rpc - An Ethereum JSON-RPC Server for Signet nodes. Makes heavy use of
  reth internals.

[Signet]: https://signet.sh
[trevm]: https://docs.rs/trevm/latest/trevm/
[Signet docs]: https://docs.signet.sh
