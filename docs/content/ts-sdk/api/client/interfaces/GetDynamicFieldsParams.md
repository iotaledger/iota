# Interface: GetDynamicFieldsParams

Return the list of dynamic field objects owned by an object.

## Properties

### parentId

> **parentId**: `string`

The ID of the parent object

---

### cursor?

> `optional` **cursor**: `null` \| `string`

An optional paging cursor. If provided, the query will start from the next item after the specified
cursor. Default to start from the first item if not specified.

---

### limit?

> `optional` **limit**: `null` \| `number`

Maximum item returned per page, default to [QUERY_MAX_RESULT_LIMIT] if not specified.
