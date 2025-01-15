# Interface: QueryEventsParams

Return list of events for a specified query criteria.

## Properties

### query

> **query**: [`IotaEventFilter`](../type-aliases/IotaEventFilter.md)

The event query criteria. See
[Event filter](https://docs.iota.org/developer/iota-101/using-events#applying-event-filters)
documentation for examples.

---

### cursor?

> `optional` **cursor**: `null` \| [`EventId`](EventId.md)

optional paging cursor

---

### limit?

> `optional` **limit**: `null` \| `number`

maximum number of items per page, default to [QUERY_MAX_RESULT_LIMIT] if not specified.

---

### order?

> `optional` **order**: `null` \| `"ascending"` \| `"descending"`

query result ordering, default to false (ascending order), oldest record first.
