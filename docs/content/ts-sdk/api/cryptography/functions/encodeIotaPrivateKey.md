# Function: encodeIotaPrivateKey()

> **encodeIotaPrivateKey**(`bytes`, `scheme`): `string`

This returns a Bech32 encoded string starting with `iotaprivkey`,
encoding 33-byte `flag || bytes` for the given the 32-byte private
key and its signature scheme.

## Parameters

• **bytes**: `Uint8Array`

• **scheme**: [`SignatureScheme`](../type-aliases/SignatureScheme.md)

## Returns

`string`
