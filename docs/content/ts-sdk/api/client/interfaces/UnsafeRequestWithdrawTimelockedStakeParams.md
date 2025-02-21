# Interface: UnsafeRequestWithdrawTimelockedStakeParams

Withdraw timelocked stake from a validator's staking pool.

## Properties

### signer

> **signer**: `string`

the transaction signer's IOTA address

---

### timelockedStakedIota

> **timelockedStakedIota**: `string`

TimelockedStakedIota object ID

---

### gas

> **gas**: `string`

gas object to be used in this transaction

---

### gasBudget

> **gasBudget**: `string`

the gas budget, the transaction will fail if the gas cost exceed the budget
