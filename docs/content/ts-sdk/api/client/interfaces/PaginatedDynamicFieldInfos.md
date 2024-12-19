# Interface: PaginatedDynamicFieldInfos

`next_cursor` points to the last item in the page; Reading with `next_cursor` will start from the
next item after `next_cursor` if `next_cursor` is `Some`, otherwise it will start from the first
item.

## Properties

### data

> **data**: [`DynamicFieldInfo`](DynamicFieldInfo.md)[]

---

### hasNextPage

> **hasNextPage**: `boolean`

---

### nextCursor?

> `optional` **nextCursor**: `null` \| `string`
