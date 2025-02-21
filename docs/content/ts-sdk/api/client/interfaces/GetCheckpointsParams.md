# Interface: GetCheckpointsParams

Return paginated list of checkpoints

## Properties

### cursor?

> `optional` **cursor**: `null` \| `string`

An optional paging cursor. If provided, the query will start from the next item after the specified
cursor. Default to start from the first item if not specified.

---

### limit?

> `optional` **limit**: `null` \| `number`

Maximum item returned per page, default to [QUERY_MAX_RESULT_LIMIT_CHECKPOINTS] if not specified.

---

### descendingOrder

> **descendingOrder**: `boolean`

query result ordering, default to false (ascending order), oldest record first.
