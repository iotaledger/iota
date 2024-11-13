# Interface: IJwkOct

[identity\_wasm](../modules/identity_wasm.md).IJwkOct

A JSON Web Key with OCT params.

## Hierarchy

- [`IJwk`](identity_wasm.IJwk.md)

- [`JwkParamsOct`](identity_wasm.JwkParamsOct.md)

  ↳ **`IJwkOct`**

## Table of contents

### Properties

- [use](identity_wasm.IJwkOct.md#use)
- [key\_ops](identity_wasm.IJwkOct.md#key_ops)
- [alg](identity_wasm.IJwkOct.md#alg)
- [kid](identity_wasm.IJwkOct.md#kid)
- [x5u](identity_wasm.IJwkOct.md#x5u)
- [x5c](identity_wasm.IJwkOct.md#x5c)
- [x5t](identity_wasm.IJwkOct.md#x5t)
- [x5t#S256](identity_wasm.IJwkOct.md#x5ts256)
- [k](identity_wasm.IJwkOct.md#k)

## Properties

### use

• `Optional` **use**: [`JwkUse`](../enums/jose_jwk_use.JwkUse.md)

Public Key Use.

Identifies the intended use of the public key.

[More Info](https://tools.ietf.org/html/rfc7517#section-4.2)

#### Inherited from

[IJwk](identity_wasm.IJwk.md).[use](identity_wasm.IJwk.md#use)

___

### key\_ops

• `Optional` **key\_ops**: [`JwkOperation`](../enums/jose_jwk_operation.JwkOperation.md)[]

Key Operations.

Identifies the operation(s) for which the key is intended to be used.

[More Info](https://tools.ietf.org/html/rfc7517#section-4.3)

#### Inherited from

[IJwk](identity_wasm.IJwk.md).[key_ops](identity_wasm.IJwk.md#key_ops)

___

### alg

• `Optional` **alg**: [`JwsAlgorithm`](../enums/jose_jws_algorithm.JwsAlgorithm.md)

Algorithm.

Identifies the algorithm intended for use with the key.

[More Info](https://tools.ietf.org/html/rfc7517#section-4.4)

#### Inherited from

[IJwk](identity_wasm.IJwk.md).[alg](identity_wasm.IJwk.md#alg)

___

### kid

• `Optional` **kid**: `string`

Key ID.

Used to match a specific key among a set of keys within a JWK Set.

[More Info](https://tools.ietf.org/html/rfc7517#section-4.5)

#### Inherited from

[IJwk](identity_wasm.IJwk.md).[kid](identity_wasm.IJwk.md#kid)

___

### x5u

• `Optional` **x5u**: `string`

X.509 URL.

A URI that refers to a resource for an X.509 public key certificate or
certificate chain.

[More Info](https://tools.ietf.org/html/rfc7517#section-4.6)

#### Inherited from

[IJwk](identity_wasm.IJwk.md).[x5u](identity_wasm.IJwk.md#x5u)

___

### x5c

• `Optional` **x5c**: `string`[]

X.509 Certificate Chain.

Contains a chain of one or more PKIX certificates.

[More Info](https://tools.ietf.org/html/rfc7517#section-4.7)

#### Inherited from

[IJwk](identity_wasm.IJwk.md).[x5c](identity_wasm.IJwk.md#x5c)

___

### x5t

• `Optional` **x5t**: `string`

X.509 Certificate SHA-1 Thumbprint.

A base64url-encoded SHA-1 thumbprint of the DER encoding of an X.509
certificate.

[More Info](https://tools.ietf.org/html/rfc7517#section-4.8)

#### Inherited from

[IJwk](identity_wasm.IJwk.md).[x5t](identity_wasm.IJwk.md#x5t)

___

### x5t#S256

• `Optional` **x5t#S256**: `string`

X.509 Certificate SHA-256 Thumbprint.

A base64url-encoded SHA-256 thumbprint of the DER encoding of an X.509
certificate.

[More Info](https://tools.ietf.org/html/rfc7517#section-4.9)

#### Inherited from

[IJwk](identity_wasm.IJwk.md).[x5t#S256](identity_wasm.IJwk.md#x5ts256)

___

### k

• **k**: `string`

The symmetric key as a base64url-encoded value.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.4.1)

#### Inherited from

[JwkParamsOct](identity_wasm.JwkParamsOct.md).[k](identity_wasm.JwkParamsOct.md#k)
