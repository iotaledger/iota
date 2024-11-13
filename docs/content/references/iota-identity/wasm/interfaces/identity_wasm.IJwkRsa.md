# Interface: IJwkRsa

[identity\_wasm](../modules/identity_wasm.md).IJwkRsa

A JSON Web Key with RSA params.

## Hierarchy

- [`IJwk`](identity_wasm.IJwk.md)

- [`JwkParamsRsa`](identity_wasm.JwkParamsRsa.md)

  ↳ **`IJwkRsa`**

## Table of contents

### Properties

- [use](identity_wasm.IJwkRsa.md#use)
- [key\_ops](identity_wasm.IJwkRsa.md#key_ops)
- [alg](identity_wasm.IJwkRsa.md#alg)
- [kid](identity_wasm.IJwkRsa.md#kid)
- [x5u](identity_wasm.IJwkRsa.md#x5u)
- [x5c](identity_wasm.IJwkRsa.md#x5c)
- [x5t](identity_wasm.IJwkRsa.md#x5t)
- [x5t#S256](identity_wasm.IJwkRsa.md#x5ts256)
- [n](identity_wasm.IJwkRsa.md#n)
- [e](identity_wasm.IJwkRsa.md#e)
- [d](identity_wasm.IJwkRsa.md#d)
- [p](identity_wasm.IJwkRsa.md#p)
- [q](identity_wasm.IJwkRsa.md#q)
- [dp](identity_wasm.IJwkRsa.md#dp)
- [dq](identity_wasm.IJwkRsa.md#dq)
- [qi](identity_wasm.IJwkRsa.md#qi)
- [oth](identity_wasm.IJwkRsa.md#oth)

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

### n

• **n**: `string`

The modulus value for the RSA public key as a base64urlUInt-encoded value.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.3.1.1)

#### Inherited from

[JwkParamsRsa](identity_wasm.JwkParamsRsa.md).[n](identity_wasm.JwkParamsRsa.md#n)

___

### e

• **e**: `string`

The exponent value for the RSA public key as a base64urlUInt-encoded value.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.3.1.2)

#### Inherited from

[JwkParamsRsa](identity_wasm.JwkParamsRsa.md).[e](identity_wasm.JwkParamsRsa.md#e)

___

### d

• `Optional` **d**: `string`

The private exponent value for the RSA private key as a base64urlUInt-encoded value.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.3.2.1)

#### Inherited from

[JwkParamsRsa](identity_wasm.JwkParamsRsa.md).[d](identity_wasm.JwkParamsRsa.md#d)

___

### p

• `Optional` **p**: `string`

The first prime factor as a base64urlUInt-encoded value.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.3.2.2)

#### Inherited from

[JwkParamsRsa](identity_wasm.JwkParamsRsa.md).[p](identity_wasm.JwkParamsRsa.md#p)

___

### q

• `Optional` **q**: `string`

The second prime factor as a base64urlUInt-encoded value.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.3.2.3)

#### Inherited from

[JwkParamsRsa](identity_wasm.JwkParamsRsa.md).[q](identity_wasm.JwkParamsRsa.md#q)

___

### dp

• `Optional` **dp**: `string`

The Chinese Remainder Theorem (CRT) exponent of the first factor as a base64urlUInt-encoded value.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.3.2.4)

#### Inherited from

[JwkParamsRsa](identity_wasm.JwkParamsRsa.md).[dp](identity_wasm.JwkParamsRsa.md#dp)

___

### dq

• `Optional` **dq**: `string`

The CRT exponent of the second factor as a base64urlUInt-encoded value.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.3.2.5)

#### Inherited from

[JwkParamsRsa](identity_wasm.JwkParamsRsa.md).[dq](identity_wasm.JwkParamsRsa.md#dq)

___

### qi

• `Optional` **qi**: `string`

The CRT coefficient of the second factor as a base64urlUInt-encoded value.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.3.2.6)

#### Inherited from

[JwkParamsRsa](identity_wasm.JwkParamsRsa.md).[qi](identity_wasm.JwkParamsRsa.md#qi)

___

### oth

• `Optional` **oth**: [`JwkParamsRsaPrime`](identity_wasm.JwkParamsRsaPrime.md)[]

An array of information about any third and subsequent primes, should they exist.

[More Info](https://tools.ietf.org/html/rfc7518#section-6.3.2.7)

#### Inherited from

[JwkParamsRsa](identity_wasm.JwkParamsRsa.md).[oth](identity_wasm.JwkParamsRsa.md#oth)
