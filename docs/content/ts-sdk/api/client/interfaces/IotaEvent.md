# Interface: IotaEvent

## Properties

### bcs

> **bcs**: `string`

Base 58 encoded bcs bytes of the move event

---

### id

> **id**: [`EventId`](EventId.md)

Sequential event ID, ie (transaction seq number, event seq number). 1) Serves as a unique event ID
for each fullnode 2) Also serves to sequence events for the purposes of pagination and querying. A
higher id is an event seen later by that fullnode. This ID is the "cursor" for event querying.

---

### packageId

> **packageId**: `string`

Move package where this event was emitted.

---

### parsedJson

> **parsedJson**: `unknown`

Parsed json value of the event

---

### sender

> **sender**: `string`

Sender's IOTA address.

---

### timestampMs?

> `optional` **timestampMs**: `null` \| `string`

UTC timestamp in milliseconds since epoch (1/1/1970)

---

### transactionModule

> **transactionModule**: `string`

Move module where this event was emitted.

---

### type

> **type**: `string`

Move event type.
