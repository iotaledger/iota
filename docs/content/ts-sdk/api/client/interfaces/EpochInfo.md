# Interface: EpochInfo

## Properties

### endOfEpochInfo?

> `optional` **endOfEpochInfo**: `null` \| [`EndOfEpochInfo`](EndOfEpochInfo.md)

The end of epoch information.

---

### epoch

> **epoch**: `string`

Epoch number

---

### epochStartTimestamp

> **epochStartTimestamp**: `string`

The timestamp when the epoch started.

---

### epochTotalTransactions

> **epochTotalTransactions**: `string`

Count of tx in epoch

---

### firstCheckpointId

> **firstCheckpointId**: `string`

First, last checkpoint sequence numbers

---

### referenceGasPrice?

> `optional` **referenceGasPrice**: `null` \| `string`

The reference gas price for the given epoch.

---

### validators

> **validators**: [`IotaValidatorSummary`](IotaValidatorSummary.md)[]

List of validators included in epoch
