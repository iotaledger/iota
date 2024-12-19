# Function: decodeIotaPrivateKey()

> **decodeIotaPrivateKey**(`value`): [`ParsedKeypair`](../type-aliases/ParsedKeypair.md)

This returns an ParsedKeypair object based by validating the
33-byte Bech32 encoded string starting with `iotaprivkey`, and
parse out the signature scheme and the private key in bytes.

## Parameters

• **value**: `string`

## Returns

[`ParsedKeypair`](../type-aliases/ParsedKeypair.md)
