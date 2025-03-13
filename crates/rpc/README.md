## signet-rpc

This crate contains the RPC server for Signet. The RPC server is a JSON-RPC
server that listens for incoming requests and processes them. The server is
built on top of the `ajj` crate, and uses the `tokio` runtime.

This crate is intended to be used as part of a complete [reth] node. It is
incredibly difficult to use this crate without a full reth node, as it requires
a database handle and access to host configuration. If you are interested in
doing that, let us know we think it'd be cool.

### What's new in Signet?

Signet's RPC server draws heavily on [reth]'s data types, and borrows code from
reth's RPC handler logic. However, we make a few design decisions that are
unique to Signet:

- The following endpoints are disabled
  - wallet-related endpoints like `eth_sign`. Good Riddance.
  - network-related endpoints like `eth_listening`. Signet has no network.
  - mining-related endpoints like `eth_mining`. Signet needs no miners.
  - txpool-related endpoints like `txpool_content`. Signet wants no txpool.
  - uncle-related endpoints like `eth_getUncleByBlockHashAndIndex`. Signet
    knows no family.
  - trie-related endpoints like `eth_getProof`. Signet grows no tries.
- Filters and subscriptions have been rewritten from the ground up.
- Bundle-related endpoints (WIP) use signet bundles from the `signet-bundle`
  crate.

See the [Signet Docs] for more information.

### What's in this crate?

- `RpcCtx` a struct managing the DB handle, subscriptions, filters, etc.
- The `router()` function will create a complete [`ajj::Router`].
- `serve_*` family of methods allow quick setup of the RPC server.

This is a work in progress. The RPC server is fully functional, but a few
things are missing.

- The following namespaces are not well-supported yet:
  - `admin_`
  - `debug_`
  - `trace_`
  - `signet_`

[reth]: https://github.com/paradigmxyz/reth
[`ajj`]: https://docs.rs/ajj/latest/ajj/
[`ajj::Router`]: https://docs.rs/ajj/latest/ajj/struct.Router.html
[`tokio`]: https://docs.rs/tokio/latest/tokio/
[Signet Docs]: https://docs.signet.sh
