# Interface: UnsafeRequestAddStakeParams

Add stake to a validator's staking pool using multiple coins and amount.

## Properties

### signer

> **signer**: `string`

the transaction signer's IOTA address

---

### coins

> **coins**: `string`[]

Coin<IOTA> object to stake

---

### amount?

> `optional` **amount**: `null` \| `string`

stake amount

---

### validator

> **validator**: `string`

the validator's IOTA address

---

### gas?

> `optional` **gas**: `null` \| `string`

gas object to be used in this transaction, node will pick one from the signer's possession if not
provided

---

### gasBudget

> **gasBudget**: `string`

the gas budget, the transaction will fail if the gas cost exceed the budget
