# Interface: UnsafeBatchTransactionParams

Create an unsigned batched transaction.

## Properties

### signer

> **signer**: `string`

the transaction signer's Iota address

---

### singleTransactionParams

> **singleTransactionParams**: [`RPCTransactionRequestParams`](../type-aliases/RPCTransactionRequestParams.md)[]

list of transaction request parameters

---

### gas?

> `optional` **gas**: `null` \| `string`

gas object to be used in this transaction, node will pick one from the signer's possession if not
provided

---

### gasBudget

> **gasBudget**: `string`

the gas budget, the transaction will fail if the gas cost exceed the budget

---

### txnBuilderMode?

> `optional` **txnBuilderMode**: `null` \| [`IotaTransactionBlockBuilderMode`](../type-aliases/IotaTransactionBlockBuilderMode.md)

Whether this is a regular transaction or a Dev Inspect Transaction
