---
description: The chain's state consists of balances of IOTA objects and a collection of key/value pairs representing use case-specific data stored in the chain by its smart contracts outside the object ledger.
image: /img/iota-evm/chain0.png
tags:
 - explanation
 - state
 - evm
teams:
 - iotaledger/l2-smart-contract
---

# State, Transitions, and State Anchoring

## State of the Chain

The state of the chain consists of:

- A ledger of accounts owning IOTA _objects_ (coins or other objects). The chain acts as a custodian for those funds on behalf of each account's owner.
- A collection of arbitrary key/value pairs (the _data state_) that contains use case-specific data stored by the smart contracts in the chain.

The chain's state is an append-only (immutable) _data structure_ maintained by the distributed consensus of its
validators.

## Digital Assets on the Chain

Each native L1 account in the IOTA object ledger is represented by an address and controlled by an entity holding the
corresponding private/public key pair.
In the object ledger, an account is a collection of objects belonging to the address.

Each ISC L2 chain has an L1 account, called the _chain account_, holding all coins and objects entrusted to the chain in a single
object, the anchor.
It is similar to how a bank holds all deposits in its vault. This way, the chain (the entity controlling the state
output) becomes a custodian for the assets owned by its clients, similar to how the bank's client owns the money
deposited in the bank.

The consolidated assets held in the chain are the _total assets on-chain_, which are contained in the state output of
the chain.

The chain account is controlled by the `ChainOwner`(the set of validators) and owns an Anchor with an Object ID, also known as _chain ID_.

## The Data State

The data state of the chain consists of a collection of key/value pairs.
Each key and each value are arbitrary byte arrays.

In its persistent form, the data state is stored in a key/value database outside the object ledger and maintained by the
validator nodes of the chain.
The state stored in the database is called the _solid state_.

While a smart contract request is being executed, the _virtual state_ is an in-memory collection of key/value pairs that
can become solid upon being committed to the database.
An essential property of the virtual state is the possibility of having several virtual states in parallel as
candidates, with a possibility for one of them to be solidified.

The data state has a state hash, a timestamp, and a state index.
The state hash is usually a Merkle root, but it can be any hashing function of all data in the data state.

The data state hash and on-chain assets are contained in a single atomic unit on the L1 ledger: the anchor object.
Each state mutation (state transition) of the chain is an atomic event that changes the on-chain assets and the data state.

## Anchoring the State

The data state is stored outside the ledger, on the distributed database maintained by _validator_ nodes.
_Anchoring the state_ means placing the hash of the data state into the anchor object and adding it to the L1 ledger.
The ledger guarantees that there is _exactly one_ such object for each chain on the ledger at every moment.
We call this object the anchor object and the containing transaction the _or
anchor transaction_ of the chain.
The anchor object is controlled by the entity running the chain.

With the anchoring mechanism, the object ledger provides the following guarantees to the IOTA Smart Contracts chain:

- There is a global consensus on the state of the chain
- The state is immutable and tamper-proof
- The state is consistent (see below)

The state output contains:

- The identity of the chain (its L1 object ID)
- The hash of the data state
- The state index, which is incremented with each new state output

## State Transitions

The data state is updated by mutations of its key/value pairs.
Each mutation either sets a value for a key or deletes a key (and the associated value).
Any update to the data state can be reduced to a partially ordered sequence of mutations.

A _block_ is a collection of mutations to the data state that is applied in a state transition:

```go
next data state = apply(current data state, block)
```

The state transition in the chain occurs atomically in an L1 transaction that mutates the Anchor object. The transaction includes the movement of the chain's assets and the update of the state hash,

At any moment in time, the data state of the chain is a result of applying the historical sequence of blocks, starting
from the empty data state.

![State transitions](/img/iota-evm/chain0.png)

On the L1 ledger, the state's history is represented as a sequence (chain) of versions of the anchor object, each holding the chain's
assets in a particular state and the anchoring hash of the data state.
Note that not all of the state's transition history may be available: due to practical reasons, older transactions may be
pruned in a snapshot process.
The only thing guaranteed is that the tip of the chain of objects is always available (which includes the latest data
state hash).

The ISC virtual machine (VM) computes the blocks and state outputs that anchor the state, which ensures that the state
transitions are calculated deterministically and consistently.

![Chain](/img/iota-evm/chain1.png)
