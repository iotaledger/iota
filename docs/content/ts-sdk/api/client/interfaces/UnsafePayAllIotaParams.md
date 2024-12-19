# Interface: UnsafePayAllIotaParams

Send all IOTA coins to one recipient. This is for IOTA coin only and does not require a separate gas
coin object. Specifically, what pay_all_iota does are: 1. accumulate all IOTA from input coins and
deposit all IOTA to the first input coin 2. transfer the updated first coin to the recipient and
also use this first coin as gas coin object. 3. the balance of the first input coin after tx is
sum(input_coins) - actual_gas_cost. 4. all other input coins other than the first are deleted.

## Properties

### signer

> **signer**: `string`

the transaction signer's Iota address

---

### inputCoins

> **inputCoins**: `string`[]

the Iota coins to be used in this transaction, including the coin for gas payment.

---

### recipient

> **recipient**: `string`

the recipient address,

---

### gasBudget

> **gasBudget**: `string`

the gas budget, the transaction will fail if the gas cost exceed the budget
