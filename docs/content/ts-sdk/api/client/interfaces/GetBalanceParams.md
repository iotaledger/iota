# Interface: GetBalanceParams

Return the total coin balance for one coin type, owned by the address owner.

## Properties

### owner

> **owner**: `string`

the owner's Iota address

---

### coinType?

> `optional` **coinType**: `null` \| `string`

optional type names for the coin (e.g., 0x168da5bf1f48dafc111b0a488fa454aca95e0b5e::usdc::USDC),
default to 0x2::iota::IOTA if not specified.
