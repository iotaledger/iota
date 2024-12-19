# Interface: GetCoinsParams

Return all Coin<`coin_type`> objects owned by an address.

## Properties

### owner

> **owner**: `string`

the owner's Iota address

---

### coinType?

> `optional` **coinType**: `null` \| `string`

optional type name for the coin (e.g., 0x168da5bf1f48dafc111b0a488fa454aca95e0b5e::usdc::USDC),
default to 0x2::iota::IOTA if not specified.

---

### cursor?

> `optional` **cursor**: `null` \| `string`

optional paging cursor

---

### limit?

> `optional` **limit**: `null` \| `number`

maximum number of items per page
