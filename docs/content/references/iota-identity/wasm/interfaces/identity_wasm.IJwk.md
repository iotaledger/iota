# Interface: IJwk

[identity\_wasm](../modules/identity_wasm.md).IJwk

A JSON Web Key.

## Hierarchy

- **`IJwk`**

  ↳ [`IJwkEc`](identity_wasm.IJwkEc.md)

  ↳ [`IJwkRsa`](identity_wasm.IJwkRsa.md)

  ↳ [`IJwkOkp`](identity_wasm.IJwkOkp.md)

  ↳ [`IJwkOct`](identity_wasm.IJwkOct.md)

## Table of contents

### Properties

- [kty](identity_wasm.IJwk.md#kty)
- [use](identity_wasm.IJwk.md#use)
- [key\_ops](identity_wasm.IJwk.md#key_ops)
- [alg](identity_wasm.IJwk.md#alg)
- [kid](identity_wasm.IJwk.md#kid)
- [x5u](identity_wasm.IJwk.md#x5u)
- [x5c](identity_wasm.IJwk.md#x5c)
- [x5t](identity_wasm.IJwk.md#x5t)
- [x5t#S256](identity_wasm.IJwk.md#x5ts256)

## Properties

### kty

• **kty**: [`JwkType`](../enums/jose_jwk_type.JwkType.md)

Key Type.

Identifies the cryptographic algorithm family used with the key.

[More Info](https://tools.ietf.org/html/rfc7517#section-4.1)

___

### use

• `Optional` **use**: [`JwkUse`](../enums/jose_jwk_use.JwkUse.md)

Public Key Use.

Identifies the intended use of the public key.

[More Info](https://tools.ietf.org/html/rfc7517#section-4.2)

___

### key\_ops

• `Optional` **key\_ops**: [`JwkOperation`](../enums/jose_jwk_operation.JwkOperation.md)[]

Key Operations.

Identifies the operation(s) for which the key is intended to be used.

[More Info](https://tools.ietf.org/html/rfc7517#section-4.3)

___

### alg

• `Optional` **alg**: [`JwsAlgorithm`](../enums/jose_jws_algorithm.JwsAlgorithm.md)

Algorithm.

Identifies the algorithm intended for use with the key.

[More Info](https://tools.ietf.org/html/rfc7517#section-4.4)

___

### kid

• `Optional` **kid**: `string`

Key ID.

Used to match a specific key among a set of keys within a JWK Set.

[More Info](https://tools.ietf.org/html/rfc7517#section-4.5)

___

### x5u

• `Optional` **x5u**: `string`

X.509 URL.

A URI that refers to a resource for an X.509 public key certificate or
certificate chain.

[More Info](https://tools.ietf.org/html/rfc7517#section-4.6)

___

### x5c

• `Optional` **x5c**: `string`[]

X.509 Certificate Chain.

Contains a chain of one or more PKIX certificates.

[More Info](https://tools.ietf.org/html/rfc7517#section-4.7)

___

### x5t

• `Optional` **x5t**: `string`

X.509 Certificate SHA-1 Thumbprint.

A base64url-encoded SHA-1 thumbprint of the DER encoding of an X.509
certificate.

[More Info](https://tools.ietf.org/html/rfc7517#section-4.8)

___

### x5t#S256

• `Optional` **x5t#S256**: `string`

X.509 Certificate SHA-256 Thumbprint.

A base64url-encoded SHA-256 thumbprint of the DER encoding of an X.509
certificate.

[More Info](https://tools.ietf.org/html/rfc7517#section-4.9)
