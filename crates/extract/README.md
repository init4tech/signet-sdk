## signet-extract

Extraction logic for Signet. This crate contains functions that traverse reth
[`Chain`] objects and extract `Signet`-specific data from them.

### What's new in Signet?

Signet nodes watch Ethereum for specific events, and use these events to
trigger actions on Signet. These actions are processed by the Signet EVM during
block processing.

There are 4 types of events that trigger actions on Signet:

- `BlockSubmitted` - Retrieve builder-submitted transactions from an Ethereum
  blob, and execute them on Signet.
- `Enter`- Mint ETH on Signet.
- `EnterToken` - Mint tokens on Signet.
- `Transact` - Execute a transaction on Signet.

Another event affects Signet's [conditional transactions]. The `Fill` event is
used to populate the aggregate fills for conditional transactions. This event is
emitted by the `Orders` contract on Ethereum when a trade is executed, and then
used to enforce the conditional invariant on Signet.

### What's in this crate?

The `Extractor` object traverses a [`Chain`] and produces a `Extracts` per
block. This object contains all relevant events that occured in the block, as
well as a populated `AggregateFills`.

[`Chain`]: https://reth.rs/docs/reth/providers/struct.Chain.html
