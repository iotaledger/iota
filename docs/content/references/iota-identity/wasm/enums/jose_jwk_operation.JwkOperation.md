# Enumeration: JwkOperation

[jose/jwk\_operation](../modules/jose_jwk_operation.md).JwkOperation

Supported algorithms for the JSON Web Key `key_ops` property.

[More Info](https://www.iana.org/assignments/jose/jose.xhtml#web-key-operations)

## Table of contents

### Enumeration Members

- [Sign](jose_jwk_operation.JwkOperation.md#sign)
- [Verify](jose_jwk_operation.JwkOperation.md#verify)
- [Encrypt](jose_jwk_operation.JwkOperation.md#encrypt)
- [Decrypt](jose_jwk_operation.JwkOperation.md#decrypt)
- [WrapKey](jose_jwk_operation.JwkOperation.md#wrapkey)
- [UnwrapKey](jose_jwk_operation.JwkOperation.md#unwrapkey)
- [DeriveKey](jose_jwk_operation.JwkOperation.md#derivekey)
- [DeriveBits](jose_jwk_operation.JwkOperation.md#derivebits)

## Enumeration Members

### Sign

• **Sign** = ``"sign"``

Compute digital signature or MAC.

___

### Verify

• **Verify** = ``"verify"``

Verify digital signature or MAC.

___

### Encrypt

• **Encrypt** = ``"encrypt"``

Encrypt content.

___

### Decrypt

• **Decrypt** = ``"decrypt"``

Decrypt content and validate decryption, if applicable.

___

### WrapKey

• **WrapKey** = ``"wrapKey"``

Encrypt key.

___

### UnwrapKey

• **UnwrapKey** = ``"unwrapKey"``

Decrypt key and validate decryption, if applicable.

___

### DeriveKey

• **DeriveKey** = ``"deriveKey"``

Derive key.

___

### DeriveBits

• **DeriveBits** = ``"deriveBits"``

Derive bits not to be used as a key.
