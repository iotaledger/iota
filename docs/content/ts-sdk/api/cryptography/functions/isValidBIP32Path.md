# Function: isValidBIP32Path()

> **isValidBIP32Path**(`path`): `boolean`

Parse and validate a path that is compliant to BIP-32 in form m/54'/4218'/{account_index}'/{change_index}/{address_index}
for Secp256k1 and m/74'/4218'/{account_index}'/{change_index}/{address_index} for Secp256r1.

Note that the purpose for Secp256k1 is registered as 54, to differentiate from Ed25519 with purpose 44.

## Parameters

• **path**: `string`

path string (e.g. `m/54'/4218'/0'/0/0`).

## Returns

`boolean`
