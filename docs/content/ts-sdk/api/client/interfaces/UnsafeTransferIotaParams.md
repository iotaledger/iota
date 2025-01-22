# Interface: UnsafeTransferIotaParams

Create an unsigned transaction to send IOTA coin object to an IOTA address. The IOTA object is also
used as the gas object.

## Properties

### signer

> **signer**: `string`

the transaction signer's IOTA address

---

### iotaObjectId

> **iotaObjectId**: `string`

the IOTA coin object to be used in this transaction

---

### gasBudget

> **gasBudget**: `string`

the gas budget, the transaction will fail if the gas cost exceed the budget

---

### recipient

> **recipient**: `string`

the recipient's IOTA address

---

### amount?

> `optional` **amount**: `null` \| `string`

the amount to be split out and transferred
