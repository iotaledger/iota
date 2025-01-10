# Interface: IotaTransactionBlockResponse

## Properties

### balanceChanges?

> `optional` **balanceChanges**: `null` \| [`BalanceChange`](BalanceChange.md)[]

---

### checkpoint?

> `optional` **checkpoint**: `null` \| `string`

The checkpoint number when this transaction was included and hence finalized. This is only returned
in the read api, not in the transaction execution api.

---

### confirmedLocalExecution?

> `optional` **confirmedLocalExecution**: `null` \| `boolean`

---

### digest

> **digest**: `string`

---

### effects?

> `optional` **effects**: `null` \| [`TransactionEffects`](../type-aliases/TransactionEffects.md)

---

### errors?

> `optional` **errors**: `string`[]

---

### events?

> `optional` **events**: `null` \| [`IotaEvent`](IotaEvent.md)[]

---

### objectChanges?

> `optional` **objectChanges**: `null` \| [`IotaObjectChange`](../type-aliases/IotaObjectChange.md)[]

---

### rawEffects?

> `optional` **rawEffects**: `number`[]

---

### rawTransaction?

> `optional` **rawTransaction**: `string`

BCS encoded [SenderSignedData] that includes input object references returns empty array if
`show_raw_transaction` is false

---

### timestampMs?

> `optional` **timestampMs**: `null` \| `string`

---

### transaction?

> `optional` **transaction**: `null` \| [`IotaTransactionBlock`](IotaTransactionBlock.md)

Transaction input data
