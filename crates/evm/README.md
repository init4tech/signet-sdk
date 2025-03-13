## signet-evm

This crate contains utilities for working with the EVM on Signet. Signet's EVM
is an extension of the EVM that supports conditional transactions by enforcing
invariants on transaction post-states. This allows users to express complex
trades across chains that execute and settle in the same block.

### What's new in Signet?

#### L1-driven actions

Signet EVM has 3 types of L1-driven actions:

- `Enter` - Mint ETH on Signet.
- `EnterToken` - Mint tokens on Signet.
- `Transact` - Execute a transaction on Signet.

These actions are triggered by Ethereum events, and are processed by the Signet
EVM during Signet block processing. Processing occurs at the end of the signet
block.

L1-driven actions ARE transactions in Signet. They have transaction hashes
(calculated) at time of execution, increment account nonces, and are available
over the RPC. Because they are not "signed" (user account authentication is
performed by Ethereum), a "magic signature" is generated for them during
execution. This signature contains data that allows the RPC and other tools to
identify the sender of the action, without a signature.

This "magic signature" contains the following information:

- The transaction hash of the Ethereum transaction that triggered the event
- The index of the event log that triggered the action in that transactions's
  receipt.
- A 1-byte flag indicating the type of action (`Enter`, `EnterToken`,
  or `Transact`).
- For `Transact` actions, the address of the sender of the transaction.

For `Enter` and `EnterToken` actions, the sender is always
`0x00000000000000000000746f6b656e61646d696e`, which is the hexadecimal
representation of the string `tokenadmin`. For `Transact` actions, the sender
is the address of the Ethereum account that triggered the action.

#### Conditional Transactions

Signet's [conditional transactions] are a way to move tokens between chains. The
conditional transactions on Signet confirm and succeed if and ONLY IF
corresponding tokens move on Ethereum and Signet. This allows users to express
complex trades across chains that execute and settle in the same block.

Signet's block validation is an extension of EVM block validation that enforces
a conditional invariant on all transactions in the block. If the transaction
emits an `Order` event, the transaction is applied to the Signet state if and
ONLY IF all of its outputs have corresponding net `Fill` events on the relevant
`Orders` contract. This invariant is enforced on a per-transaction basis.

Ethereum `Fill` events are indexed before Signet block processing starts, while
Signet `Fill` events are indexed on a running basis during block processing. As
transactions execute, the `Fill` amounts are consumed. If any transaction
attempts to consume more `Fill` than is available, that transaction is invalid.

For example, if Alice creates an `Order` that has an `Input` of 1 ETH, and an
output of 0.5 ETH on Ethereum, and 500 USDC on Signet, that indicates that the
transaction must consume 0.5 ETH of `Fill` amount on Ethereum and 500 USDC on
Signet. The transaction containing that order is effective if and ONLY IF
Ethereum's `Orders` contract has a corresponding `Fill` event with 0.5 ETH and
Signet's `Orders` contract has a corresponding `Fill` event with 500 USDC AND
no prior transaction has consumed those `Fill` amounts.

#### Signet's block processing order

The Signet EVM consumes events from Ethereum, and creates system transactions.
In order to maintain block simulation, this is done at the END of every Signet
block. Specifically, things occur in the following order:

- The builder-supplied block is processed by the Signet EVM:
  - Each transaction is checked for validity.
  - Each transaction is simulated against the Signet state.
  - The outcome of the simulation is checked for conformity with the
    conditional invariants.
  - If accepted, `Fill` amount state and Signet state are updated.
- Ethereum events are processed by the Signet EVM
  - `Enter` events are processed, minting ETH.
  - `EnterToken` events are processed, minting tokens.
  - `Transact` events are processed, executing [L1-driven transactions].
  - The balance of the `RollupPassage` contract is deleted.
  - The block's `baseFee` is credited to the appropriate account.

### What's in this crate

- `SignetDriver` - a [trevm] driver capable of executing `Signet` blocks based
  on Ethereum extracts.
- `OrderDetector` a [revm] inspector that detects `Order` events triggered
  during Signet EVM execution.
- A set of [trevm] type aliases for Signet's EVM.
- `ToReth` - Util trait for type conversions necessary to work with reth's
  database model.

[L1-driven transactions]: https://docs.signet.sh/learn-about-signet/cross-chain-transfers-on-signet#moving-from-ethereum-to-signet
[conditional transactions]: https://docs.signet.sh/learn-about-signet/cross-chain-transfers-on-signet
