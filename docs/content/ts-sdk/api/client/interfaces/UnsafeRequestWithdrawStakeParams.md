# Interface: UnsafeRequestWithdrawStakeParams

Withdraw stake from a validator's staking pool.

## Properties

### signer

> **signer**: `string`

the transaction signer's IOTA address

---

### stakedIota

> **stakedIota**: `string`

StakedIota object ID

---

### gas?

> `optional` **gas**: `null` \| `string`

gas object to be used in this transaction, node will pick one from the signer's possession if not
provided

---

### gasBudget

> **gasBudget**: `string`

the gas budget, the transaction will fail if the gas cost exceed the budget
