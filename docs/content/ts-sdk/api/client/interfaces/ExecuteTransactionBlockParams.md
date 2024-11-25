# Interface: ExecuteTransactionBlockParams

Execute the transaction and wait for results if desired. Request types: 1. WaitForEffectsCert: waits
for TransactionEffectsCert and then return to client. This mode is a proxy for transaction
finality. 2. WaitForLocalExecution: waits for TransactionEffectsCert and make sure the node executed
the transaction locally before returning the client. The local execution makes sure this node is
aware of this transaction when client fires subsequent queries. However if the node fails to execute
the transaction locally in a timely manner, a bool type in the response is set to false to indicated
the case. request_type is default to be `WaitForEffectsCert` unless options.show_events or
options.show_effects is true

## Properties

### transactionBlock

> **transactionBlock**: `string` \| `Uint8Array`

BCS serialized transaction data bytes without its type tag, as base-64 encoded string.

---

### signature

> **signature**: `string` \| `string`[]

A list of signatures (`flag || signature || pubkey` bytes, as base-64 encoded string). Signature is
committed to the intent message of the transaction data, as base-64 encoded string.

---

### options?

> `optional` **options**: `null` \| [`IotaTransactionBlockResponseOptions`](IotaTransactionBlockResponseOptions.md)

options for specifying the content to be returned

---

### ~~requestType?~~

> `optional` **requestType**: `null` \| [`ExecuteTransactionRequestType`](../type-aliases/ExecuteTransactionRequestType.md)

#### Deprecated

requestType will be ignored by JSON RPC in the future
