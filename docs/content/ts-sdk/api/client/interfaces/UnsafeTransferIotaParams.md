# Interface: UnsafeTransferIotaParams

Create an unsigned transaction to send IOTA coin object to a Iota address. The IOTA object is also
used as the gas object.

## Properties

### signer

> **signer**: `string`

the transaction signer's Iota address

---

### iotaObjectId

> **iotaObjectId**: `string`

the Iota coin object to be used in this transaction

---

### gasBudget

> **gasBudget**: `string`

the gas budget, the transaction will fail if the gas cost exceed the budget

---

### recipient

> **recipient**: `string`

the recipient's Iota address

---

### amount?

> `optional` **amount**: `null` \| `string`

the amount to be split out and transferred
