# Interface: JwkParamsEc

[identity\_wasm](../modules/identity_wasm.md).JwkParamsEc

Parameters for Elliptic Curve Keys.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.2)

## Hierarchy

- **`JwkParamsEc`**

  ↳ [`IJwkEc`](identity_wasm.IJwkEc.md)

## Table of contents

### Properties

- [crv](identity_wasm.JwkParamsEc.md#crv)
- [x](identity_wasm.JwkParamsEc.md#x)
- [y](identity_wasm.JwkParamsEc.md#y)
- [d](identity_wasm.JwkParamsEc.md#d)

## Properties

### crv

• **crv**: `string`

Identifies the cryptographic curve used with the key.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.2.1.1)

___

### x

• **x**: `string`

The `x` coordinate for the Elliptic Curve point as a base64url-encoded value.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.2.1.2)

___

### y

• **y**: `string`

The `y` coordinate for the Elliptic Curve point as a base64url-encoded value.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.2.1.3)

___

### d

• `Optional` **d**: `string`

The Elliptic Curve private key as a base64url-encoded value.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.2.2.1)
