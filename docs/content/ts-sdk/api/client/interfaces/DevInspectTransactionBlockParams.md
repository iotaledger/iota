# Interface: DevInspectTransactionBlockParams

Runs the transaction in dev-inspect mode. Which allows for nearly any transaction (or Move call)
with any arguments. Detailed results are provided, including both the transaction effects and any
return values.

## Properties

### sender

> **sender**: `string`

---

### transactionBlock

> **transactionBlock**: `string` \| `Uint8Array` \| [`Transaction`](../../transactions/classes/Transaction.md)

BCS encoded TransactionKind(as opposed to TransactionData, which include gasBudget and gasPrice)

---

### gasPrice?

> `optional` **gasPrice**: `null` \| `number` \| `bigint`

Gas is not charged, but gas usage is still calculated. Default to use reference gas price

---

### epoch?

> `optional` **epoch**: `null` \| `string`

The epoch to perform the call. Will be set from the system state object if not provided

---

### additionalArgs?

> `optional` **additionalArgs**: `null` \| [`DevInspectArgs`](DevInspectArgs.md)

Additional arguments including gas_budget, gas_objects, gas_sponsor and skip_checks.
