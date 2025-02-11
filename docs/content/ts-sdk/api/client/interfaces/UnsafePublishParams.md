# Interface: UnsafePublishParams

Create an unsigned transaction to publish a Move package.

## Properties

### sender

> **sender**: `string`

the transaction signer's IOTA address

---

### compiledModules

> **compiledModules**: `string`[]

the compiled bytes of a Move package

---

### dependencies

> **dependencies**: `string`[]

a list of transitive dependency addresses that this set of modules depends on.

---

### gas?

> `optional` **gas**: `null` \| `string`

gas object to be used in this transaction, node will pick one from the signer's possession if not
provided

---

### gasBudget

> **gasBudget**: `string`

the gas budget, the transaction will fail if the gas cost exceed the budget
