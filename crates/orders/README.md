## signet-orders

Utilities for placing and filling [orders] on Signet.

### What's in this crate?

**Sending orders:**

- [`OrderSender`] — high-level interface for signing and submitting off-chain
  orders. Generic over any alloy `Signer` and any `OrderSubmitter` backend.
  Supports signing `UnsignedOrder`s, on-chain `Order` structs, and combined
  sign-and-send in a single call.

**Filling orders:**

- [`Filler`] — orchestrates the order-filling pipeline: fetch pending orders
  from an `OrderSource`, sign Permit2 fills, and submit them via a
  `FillSubmitter`. Returns a stream of orders and supports batch filling.
- [`FeePolicySubmitter`] — a `FillSubmitter` that builds fill and initiate
  transactions, wraps them in a `SignetEthBundle`, and submits via a
  `BundleSubmitter`. Handles gas pricing for both rollup and host chains.
- [`FillerOptions`] — configure fill signing: Permit2 deadline offset and nonce.

**Traits:**

- [`OrderSubmitter`] — submit signed orders to a backend
- [`OrderSource`] — fetch orders as a stream (with automatic pagination)
- [`FillSubmitter`] — submit signed fills (decouples `Filler` from fee/tx logic)
- [`BundleSubmitter`] — submit `SignetEthBundle`s to a backend
- [`TxBuilder`] — abstract over alloy's `FillProvider` for transaction filling

Ready-made implementations of `OrderSubmitter`, `OrderSource`, and
`BundleSubmitter` are provided for [`TxCache`] from `signet-tx-cache`.

### Usage

Add the crate to your project:

```bash
cargo add signet-orders
```

**Sending an order:**

```rust
use signet_constants::parmigiana;
use signet_orders::OrderSender;
use signet_tx_cache::TxCache;

let order_sender = OrderSender::new(signer, TxCache::parmigiana(), parmigiana::system_constants());

// Sign and submit in one call
let signed = order_sender.sign_and_send_order(order).await?;
```

**Filling orders:**

```rust
use signet_orders::{Filler, FeePolicySubmitter, FillerOptions};

let submitter = FeePolicySubmitter::new(ru_provider, host_provider, tx_cache.clone(), constants.clone());
let filler = Filler::new(signer, tx_cache, submitter, constants, FillerOptions::new());

// Fetch and fill
let orders: Vec<_> = filler.get_orders().try_collect().await?;
let response = filler.fill(orders).await?;
```

For a complete example of a filler service, see [signet-filler].

### Documentation

- [Working with Orders][orders] — overview of the Signet orders system
- [API reference]

### License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or
[MIT License](../../LICENSE-MIT) at your option.

[orders]: https://signet.sh/docs/build-on-signet/signet-to-ethereum/orders/
[API reference]: https://docs.rs/signet-orders/latest/signet_orders/
[signet-filler]: https://github.com/init4tech/signet-filler
[`OrderSender`]: https://docs.rs/signet-orders/latest/signet_orders/struct.OrderSender.html
[`Filler`]: https://docs.rs/signet-orders/latest/signet_orders/struct.Filler.html
[`FeePolicySubmitter`]: https://docs.rs/signet-orders/latest/signet_orders/struct.FeePolicySubmitter.html
[`FillerOptions`]: https://docs.rs/signet-orders/latest/signet_orders/struct.FillerOptions.html
[`OrderSubmitter`]: https://docs.rs/signet-orders/latest/signet_orders/trait.OrderSubmitter.html
[`OrderSource`]: https://docs.rs/signet-orders/latest/signet_orders/trait.OrderSource.html
[`FillSubmitter`]: https://docs.rs/signet-orders/latest/signet_orders/trait.FillSubmitter.html
[`BundleSubmitter`]: https://docs.rs/signet-orders/latest/signet_orders/trait.BundleSubmitter.html
[`TxBuilder`]: https://docs.rs/signet-orders/latest/signet_orders/trait.TxBuilder.html
[`TxCache`]: https://docs.rs/signet-tx-cache/latest/signet_tx_cache/struct.TxCache.html
