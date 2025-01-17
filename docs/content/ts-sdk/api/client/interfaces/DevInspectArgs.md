# Interface: DevInspectArgs

Additional arguments supplied to dev inspect beyond what is allowed in today's API.

## Properties

### gasBudget?

> `optional` **gasBudget**: `null` \| `string`

The gas budget for the transaction.

---

### gasObjects?

> `optional` **gasObjects**: `null` \| [`string`, `string`, `string`][]

The gas objects used to pay for the transaction.

---

### gasSponsor?

> `optional` **gasSponsor**: `null` \| `string`

The sponsor of the gas for the transaction, might be different from the sender.

---

### showRawTxnDataAndEffects?

> `optional` **showRawTxnDataAndEffects**: `null` \| `boolean`

Whether to return the raw transaction data and effects.

---

### skipChecks?

> `optional` **skipChecks**: `null` \| `boolean`

Whether to skip transaction checks for the transaction.
