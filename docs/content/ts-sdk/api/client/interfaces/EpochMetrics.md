# Interface: EpochMetrics

A light-weight version of `EpochInfo` for faster loading

## Properties

### endOfEpochInfo?

> `optional` **endOfEpochInfo**: `null` \| [`EndOfEpochInfo`](EndOfEpochInfo.md)

The end of epoch information.

---

### epoch

> **epoch**: `string`

The current epoch ID.

---

### epochStartTimestamp

> **epochStartTimestamp**: `string`

The timestamp when the epoch started.

---

### epochTotalTransactions

> **epochTotalTransactions**: `string`

The total number of transactions in the epoch.

---

### firstCheckpointId

> **firstCheckpointId**: `string`

The first checkpoint ID of the epoch.
