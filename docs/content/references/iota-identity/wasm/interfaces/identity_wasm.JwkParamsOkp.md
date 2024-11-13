# Interface: JwkParamsOkp

[identity\_wasm](../modules/identity_wasm.md).JwkParamsOkp

Parameters for Octet Key Pairs.

[More Info](https://tools.ietf.org/html/rfc8037#section-2)

## Hierarchy

- **`JwkParamsOkp`**

  ↳ [`IJwkOkp`](identity_wasm.IJwkOkp.md)

## Table of contents

### Properties

- [crv](identity_wasm.JwkParamsOkp.md#crv)
- [x](identity_wasm.JwkParamsOkp.md#x)
- [d](identity_wasm.JwkParamsOkp.md#d)

## Properties

### crv

• **crv**: `string`

The subtype of the key pair.

[More Info](https://tools.ietf.org/html/rfc8037#section-2)

___

### x

• **x**: `string`

The public key as a base64url-encoded value.

[More Info](https://tools.ietf.org/html/rfc8037#section-2)

___

### d

• `Optional` **d**: `string`

The private key as a base64url-encoded value.

[More Info](https://tools.ietf.org/html/rfc8037#section-2)
