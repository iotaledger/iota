# Interface: MultiGetTransactionBlocksParams

Returns an ordered list of transaction responses The method will throw an error if the input
contains any duplicate or the input size exceeds QUERY_MAX_RESULT_LIMIT

## Properties

### digests

> **digests**: `string`[]

A list of transaction digests.

---

### options?

> `optional` **options**: `null` \| [`IotaTransactionBlockResponseOptions`](IotaTransactionBlockResponseOptions.md)

config options to control which fields to fetch
