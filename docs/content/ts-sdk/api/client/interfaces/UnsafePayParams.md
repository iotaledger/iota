# Interface: UnsafePayParams

Send `Coin<T>` to a list of addresses, where `T` can be any coin type, following a list of amounts,
The object specified in the `gas` field will be used to pay the gas fee for the transaction. The gas
object can not appear in `input_coins`. If the gas object is not specified, the RPC server will
auto-select one.

## Properties

### signer

> **signer**: `string`

the transaction signer's Iota address

---

### inputCoins

> **inputCoins**: `string`[]

the Iota coins to be used in this transaction

---

### recipients

> **recipients**: `string`[]

the recipients' addresses, the length of this vector must be the same as amounts.

---

### amounts

> **amounts**: `string`[]

the amounts to be transferred to recipients, following the same order

---

### gas?

> `optional` **gas**: `null` \| `string`

gas object to be used in this transaction, node will pick one from the signer's possession if not
provided

---

### gasBudget

> **gasBudget**: `string`

the gas budget, the transaction will fail if the gas cost exceed the budget
