# Interface: JwkParamsRsa

[identity\_wasm](../modules/identity_wasm.md).JwkParamsRsa

Parameters for RSA Keys.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.3)

## Hierarchy

- **`JwkParamsRsa`**

  ↳ [`IJwkRsa`](identity_wasm.IJwkRsa.md)

## Table of contents

### Properties

- [n](identity_wasm.JwkParamsRsa.md#n)
- [e](identity_wasm.JwkParamsRsa.md#e)
- [d](identity_wasm.JwkParamsRsa.md#d)
- [p](identity_wasm.JwkParamsRsa.md#p)
- [q](identity_wasm.JwkParamsRsa.md#q)
- [dp](identity_wasm.JwkParamsRsa.md#dp)
- [dq](identity_wasm.JwkParamsRsa.md#dq)
- [qi](identity_wasm.JwkParamsRsa.md#qi)
- [oth](identity_wasm.JwkParamsRsa.md#oth)

## Properties

### n

• **n**: `string`

The modulus value for the RSA public key as a base64urlUInt-encoded value.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.3.1.1)

___

### e

• **e**: `string`

The exponent value for the RSA public key as a base64urlUInt-encoded value.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.3.1.2)

___

### d

• `Optional` **d**: `string`

The private exponent value for the RSA private key as a base64urlUInt-encoded value.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.3.2.1)

___

### p

• `Optional` **p**: `string`

The first prime factor as a base64urlUInt-encoded value.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.3.2.2)

___

### q

• `Optional` **q**: `string`

The second prime factor as a base64urlUInt-encoded value.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.3.2.3)

___

### dp

• `Optional` **dp**: `string`

The Chinese Remainder Theorem (CRT) exponent of the first factor as a base64urlUInt-encoded value.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.3.2.4)

___

### dq

• `Optional` **dq**: `string`

The CRT exponent of the second factor as a base64urlUInt-encoded value.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.3.2.5)

___

### qi

• `Optional` **qi**: `string`

The CRT coefficient of the second factor as a base64urlUInt-encoded value.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.3.2.6)

___

### oth

• `Optional` **oth**: [`JwkParamsRsaPrime`](identity_wasm.JwkParamsRsaPrime.md)[]

An array of information about any third and subsequent primes, should they exist.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.3.2.7)
