# Function: normalizeIotaAddress()

> **normalizeIotaAddress**(`value`, `forceAdd0x`): `string`

Perform the following operations:

1. Make the address lower case
2. Prepend `0x` if the string does not start with `0x`.
3. Add more zeros if the length of the address(excluding `0x`) is less than `IOTA_ADDRESS_LENGTH`

WARNING: if the address value itself starts with `0x`, e.g., `0x0x`, the default behavior
is to treat the first `0x` not as part of the address. The default behavior can be overridden by
setting `forceAdd0x` to true

## Parameters

• **value**: `string`

• **forceAdd0x**: `boolean` = `false`

## Returns

`string`
