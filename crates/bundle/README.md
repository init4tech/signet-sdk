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

## Refund Fields

Signet bundles support refund parameters equivalent to [Flashbots refund semantics].
These fields allow builders to return a portion of the bundle's profit to the
searcher based on execution outcomes.

### Refund Parameters

| Field | Type | Description |
|-------|------|-------------|
| `refundPercent` | `u8` (0-100) | Percentage of bundle profit to refund. Default: 90% |
| `refundRecipient` | `Address` | Address to receive the refund. Default: first tx signer |
| `refundTxHashes` | `Vec<TxHash>` | Specific txs to use for refund calculation. Default: all txs |

### How Refunds Work

1. **Profit Calculation**: The builder calculates the bundle's profit based on
   the difference between coinbase payments and gas costs.

2. **Refund Percentage**: The `refundPercent` (0-100) specifies what portion of
   this profit should be returned to the searcher. If not specified, builders
   typically default to 90%.

3. **Recipient**: The `refundRecipient` specifies where the refund is sent. If
   not specified, it defaults to the signer of the first transaction in the
   bundle.

4. **Transaction Selection**: The `refundTxHashes` field allows searchers to
   specify which transactions should be used when calculating the refund
   amount. If empty, all bundle transactions are considered.

### Example JSON

```json
{
  "txs": ["0x02f8...signed_tx_1", "0x02f8...signed_tx_2"],
  "blockNumber": "0xbc614e",
  "refundPercent": 90,
  "refundRecipient": "0x742d35Cc6634C0532925a3b844Bc9e7595f8aB12",
  "refundTxHashes": ["0xabc123...tx_hash"]
}
```

### Validation

The SDK provides validation methods for refund fields:

- `is_valid_refund_percent()` - Ensures percent is 0-100
- `is_valid_refund_tx_hashes()` - Ensures referenced tx hashes exist in bundle
- `is_valid_refunds()` - Validates all refund fields together

### Effective Defaults

The `RecoveredBundle` provides methods to get effective values with Flashbots-compatible
defaults:

- `effective_refund_percent()` - Returns specified value or default of 90%
- `effective_refund_recipient()` - Returns specified address or first tx signer

[trevm]: https://docs.rs/trevm/latest/trevm/
[Flashbots bundles]: https://docs.flashbots.net/flashbots-auction/advanced/understanding-bundles
[Flashbots refund semantics]: https://docs.flashbots.net/flashbots-auction/advanced/rpc-endpoint
[conditional transactions]: https://signet.sh/docs/learn-about-signet/cross-chain-transfers/
