# Interface: UnsafeMergeCoinsParams

Create an unsigned transaction to merge multiple coins into one coin.

## Properties

### signer

> **signer**: `string`

the transaction signer's IOTA address

---

### primaryCoin

> **primaryCoin**: `string`

the coin object to merge into, this coin will remain after the transaction

---

### coinToMerge

> **coinToMerge**: `string`

the coin object to be merged, this coin will be destroyed, the balance will be added to
`primary_coin`

---

### gas?

> `optional` **gas**: `null` \| `string`

gas object to be used in this transaction, node will pick one from the signer's possession if not
provided

---

### gasBudget

> **gasBudget**: `string`

the gas budget, the transaction will fail if the gas cost exceed the budget
