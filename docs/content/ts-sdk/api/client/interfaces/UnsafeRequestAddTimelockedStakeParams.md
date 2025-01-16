# Interface: UnsafeRequestAddTimelockedStakeParams

Add timelocked stake to a validator's staking pool using multiple balances and amount.

## Properties

### signer

> **signer**: `string`

the transaction signer's IOTA address

---

### lockedBalance

> **lockedBalance**: `string`

TimeLock<Balance<IOTA>> object to stake

---

### validator

> **validator**: `string`

the validator's IOTA address

---

### gas

> **gas**: `string`

gas object to be used in this transaction

---

### gasBudget

> **gasBudget**: `string`

the gas budget, the transaction will fail if the gas cost exceed the budget
