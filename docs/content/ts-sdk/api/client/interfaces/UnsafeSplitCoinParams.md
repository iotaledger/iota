# Interface: UnsafeSplitCoinParams

Create an unsigned transaction to split a coin object into multiple coins.

## Properties

### signer

> **signer**: `string`

the transaction signer's Iota address

---

### coinObjectId

> **coinObjectId**: `string`

the coin object to be spilt

---

### splitAmounts

> **splitAmounts**: `string`[]

the amounts to split out from the coin

---

### gas?

> `optional` **gas**: `null` \| `string`

gas object to be used in this transaction, node will pick one from the signer's possession if not
provided

---

### gasBudget

> **gasBudget**: `string`

the gas budget, the transaction will fail if the gas cost exceed the budget
