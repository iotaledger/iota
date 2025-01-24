# Interface: UnsafeSplitCoinEqualParams

Create an unsigned transaction to split a coin object into multiple equal-size coins.

## Properties

### signer

> **signer**: `string`

the transaction signer's IOTA address

---

### coinObjectId

> **coinObjectId**: `string`

the coin object to be spilt

---

### splitCount

> **splitCount**: `string`

the number of coins to split into

---

### gas?

> `optional` **gas**: `null` \| `string`

gas object to be used in this transaction, node will pick one from the signer's possession if not
provided

---

### gasBudget

> **gasBudget**: `string`

the gas budget, the transaction will fail if the gas cost exceed the budget
