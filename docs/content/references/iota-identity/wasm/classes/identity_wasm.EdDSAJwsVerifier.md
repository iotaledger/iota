# Class: EdDSAJwsVerifier

[identity\_wasm](../modules/identity_wasm.md).EdDSAJwsVerifier

An implementor of `IJwsVerifier` that can handle the
`EdDSA` algorithm.

## Table of contents

### Constructors

- [constructor](identity_wasm.EdDSAJwsVerifier.md#constructor)

### Methods

- [verify](identity_wasm.EdDSAJwsVerifier.md#verify)

## Constructors

### constructor

• **new EdDSAJwsVerifier**()

Constructs an EdDSAJwsVerifier.

## Methods

### verify

▸ **verify**(`alg`, `signingInput`, `decodedSignature`, `publicKey`): `void`

Verify a JWS signature secured with the `EdDSA` algorithm.
Only the `Ed25519` curve is supported for now.

This function is useful when one is building an `IJwsVerifier` that extends the default provided by
the IOTA Identity Framework.

# Warning

This function does not check whether `alg = EdDSA` in the protected header. Callers are expected to assert this
prior to calling the function.

#### Parameters

| Name | Type |
| :------ | :------ |
| `alg` | [`JwsAlgorithm`](../enums/jose_jws_algorithm.JwsAlgorithm.md) |
| `signingInput` | `Uint8Array` |
| `decodedSignature` | `Uint8Array` |
| `publicKey` | [`Jwk`](identity_wasm.Jwk.md) |

#### Returns

`void`
