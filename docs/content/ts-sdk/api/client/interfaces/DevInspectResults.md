# Interface: DevInspectResults

The response from processing a dev inspect transaction

## Properties

### effects

> **effects**: [`TransactionEffects`](../type-aliases/TransactionEffects.md)

Summary of effects that likely would be generated if the transaction is actually run. Note however,
that not all dev-inspect transactions are actually usable as transactions so it might not be
possible actually generate these effects from a normal transaction.

---

### error?

> `optional` **error**: `null` \| `string`

Execution error from executing the transactions

---

### events

> **events**: [`IotaEvent`](IotaEvent.md)[]

Events that likely would be generated if the transaction is actually run.

---

### rawEffects?

> `optional` **rawEffects**: `number`[]

The raw effects of the transaction that was dev inspected.

---

### rawTxnData?

> `optional` **rawTxnData**: `number`[]

The raw transaction data that was dev inspected.

---

### results?

> `optional` **results**: `null` \| [`IotaExecutionResult`](IotaExecutionResult.md)[]

Execution results (including return values) from executing the transactions
