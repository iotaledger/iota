# Type Alias: QueryTransactionBlocksParams

> **QueryTransactionBlocksParams**: `object` & [`IotaTransactionBlockResponseQuery`](../interfaces/IotaTransactionBlockResponseQuery.md)

Return list of transactions for a specified query criteria.

## Type declaration

### cursor?

> `optional` **cursor**: `string` \| `null`

An optional paging cursor. If provided, the query will start from the next item after the specified
cursor. Default to start from the first item if not specified.

### limit?

> `optional` **limit**: `number` \| `null`

Maximum item returned per page, default to QUERY_MAX_RESULT_LIMIT if not specified.

### order?

> `optional` **order**: `"ascending"` \| `"descending"` \| `null`

query result ordering, default to false (ascending order), oldest record first.
