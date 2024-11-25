# Interface: UnsafePayIotaParams

Send IOTA coins to a list of addresses, following a list of amounts. This is for IOTA coin only and
does not require a separate gas coin object. Specifically, what pay_iota does are: 1. debit each
input_coin to create new coin following the order of amounts and assign it to the corresponding
recipient. 2. accumulate all residual IOTA from input coins left and deposit all IOTA to the first
input coin, then use the first input coin as the gas coin object. 3. the balance of the first input
coin after tx is sum(input_coins) - sum(amounts) - actual_gas_cost 4. all other input coints other
than the first one are deleted.

## Properties

### signer

> **signer**: `string`

the transaction signer's Iota address

---

### inputCoins

> **inputCoins**: `string`[]

the Iota coins to be used in this transaction, including the coin for gas payment.

---

### recipients

> **recipients**: `string`[]

the recipients' addresses, the length of this vector must be the same as amounts.

---

### amounts

> **amounts**: `string`[]

the amounts to be transferred to recipients, following the same order

---

### gasBudget

> **gasBudget**: `string`

the gas budget, the transaction will fail if the gas cost exceed the budget
