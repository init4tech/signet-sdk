## signet-bundle

This crate contains utilities for working with flashbots-style bundles. Bundles
are sets of transactions that are submitted to a builder as a unit. Builders
are required to treat bundles as atomic, meaning that transactions must be
included and succeed to fail as a unit.

### What's new in Signet?

Signet bundles behave like [Flashbots bundles], however, Signet bundles contain
Ethereum token movements in addition to Signet transactions.

Signet's [conditional transactions] are a way to move tokens between chains. The
conditional transactions on Signet confirm and succeed if and ONLY IF
corresponding tokens move on Ethereum and Signet. This allows users to express
complex trades across chains that execute and settle in the same block.

These conditional transactions require specialized handling by builders, and
custom simulation logic. This crate provides utilities for simulating and
validating the effects of conditional transactions.

## What's in this crate?

This allows users and builders to simulate and validated the effects of
[conditional transactions]. This crate provides

- Bundle formats via `SignetCallBundle` and `SignetEthBundle`
- JSON-RPC response formats for the bundle endpoints.
  - `signet_simBundle` via `SignetCallBundleResponse`.
  - `signet_sendBundle` via `SignetEthBundleResponse`.
- A [trevm] driver capable of simulating `SignetCallBundle` and producing a
  `SignetCallBundleResponse`.

[trevm]: https://docs.rs/trevm/latest/trevm/
[Flashbots bundles]: https://docs.flashbots.net/flashbots-auction/advanced/understanding-bundles
[conditional transactions]: https://signet.sh/docs/learn-about-signet/cross-chain-transfers/
