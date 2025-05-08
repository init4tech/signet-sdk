## signet-types

This crate contains shared datastructures that are used by multiple Signet
components. It is used by `signet-extractor`, `signet-evm`, `signet-node` and
other ecosystem crates to share common functionality. It's pretty boring
honestly.

## What's in this crate?

- `mod constants` - contains `SignetSystemConstants` a configuration object that
  holds the system constants and is used by the extractor, the EVM, and the
  node.
- `AggregateFills` - a struct that holds the aggregate fill status for
  [conditional transactions].
- `MagicSig` - a struct that holds magic signatures for Signet L1-driven
  actions.
- `SignRequest` and `SignResponse` - data structures for block-builders
  communicating with the Signet sequencer.

[conditional transactions]: https://docs.signet.sh/learn-about-signet/cross-chain-transfers-on-signet
