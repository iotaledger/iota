# Type Alias: GetOwnedObjectsParams

> **GetOwnedObjectsParams**: `object` & [`IotaObjectResponseQuery`](../interfaces/IotaObjectResponseQuery.md)

Return the list of objects owned by an address. Note that if the address owns more than
`QUERY_MAX_RESULT_LIMIT` objects, the pagination is not accurate, because previous page may have
been updated when the next page is fetched. Please use iotax_queryObjects if this is a concern.

## Type declaration

### owner

> **owner**: `string`

the owner's IOTA address

### cursor?

> `optional` **cursor**: `string` \| `null`

An optional paging cursor. If provided, the query will start from the next item after the specified
cursor. Default to start from the first item if not specified.

### limit?

> `optional` **limit**: `number` \| `null`

Max number of items returned per page, default to [QUERY_MAX_RESULT_LIMIT] if not specified.
