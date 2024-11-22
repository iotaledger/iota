# Interface: EndOfEpochData

## Properties

### epochCommitments

> **epochCommitments**: [`CheckpointCommitment`](../type-aliases/CheckpointCommitment.md)[]

Commitments to epoch specific state (e.g. live object set)

---

### epochSupplyChange

> **epochSupplyChange**: `number`

The number of tokens that were minted (if positive) or burnt (if negative) in this epoch.

---

### nextEpochCommittee

> **nextEpochCommittee**: [`string`, `string`][]

next_epoch_committee is `Some` if and only if the current checkpoint is the last checkpoint of an
epoch. Therefore next_epoch_committee can be used to pick the last checkpoint of an epoch, which is
often useful to get epoch level summary stats like total gas cost of an epoch, or the total number
of transactions from genesis to the end of an epoch. The committee is stored as a vector of
validator pub key and stake pairs. The vector should be sorted based on the Committee data
structure.

---

### nextEpochProtocolVersion

> **nextEpochProtocolVersion**: `string`

The protocol version that is in effect during the epoch that starts immediately after this
checkpoint.
