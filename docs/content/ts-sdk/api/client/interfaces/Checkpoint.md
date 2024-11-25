# Interface: Checkpoint

## Properties

### checkpointCommitments

> **checkpointCommitments**: [`CheckpointCommitment`](../type-aliases/CheckpointCommitment.md)[]

Commitments to checkpoint state

---

### digest

> **digest**: `string`

Checkpoint digest

---

### endOfEpochData?

> `optional` **endOfEpochData**: `null` \| [`EndOfEpochData`](EndOfEpochData.md)

Present only on the final checkpoint of the epoch.

---

### epoch

> **epoch**: `string`

Checkpoint's epoch ID

---

### epochRollingGasCostSummary

> **epochRollingGasCostSummary**: [`GasCostSummary`](GasCostSummary.md)

The running total gas costs of all transactions included in the current epoch so far until this
checkpoint.

---

### networkTotalTransactions

> **networkTotalTransactions**: `string`

Total number of transactions committed since genesis, including those in this checkpoint.

---

### previousDigest?

> `optional` **previousDigest**: `null` \| `string`

Digest of the previous checkpoint

---

### sequenceNumber

> **sequenceNumber**: `string`

Checkpoint sequence number

---

### timestampMs

> **timestampMs**: `string`

Timestamp of the checkpoint - number of milliseconds from the Unix epoch Checkpoint timestamps are
monotonic, but not strongly monotonic - subsequent checkpoints can have same timestamp if they
originate from the same underlining consensus commit

---

### transactions

> **transactions**: `string`[]

Transaction digests

---

### validatorSignature

> **validatorSignature**: `string`

Validator Signature
