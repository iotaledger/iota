# Interface: UnsafeTransferObjectParams

Create an unsigned transaction to transfer an object from one address to another. The object's type
must allow public transfers

## Properties

### signer

> **signer**: `string`

the transaction signer's Iota address

---

### objectId

> **objectId**: `string`

the ID of the object to be transferred

---

### gas?

> `optional` **gas**: `null` \| `string`

gas object to be used in this transaction, node will pick one from the signer's possession if not
provided

---

### gasBudget

> **gasBudget**: `string`

the gas budget, the transaction will fail if the gas cost exceed the budget

---

### recipient

> **recipient**: `string`

the recipient's Iota address
