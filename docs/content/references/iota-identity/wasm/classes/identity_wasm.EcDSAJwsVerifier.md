# Class: EcDSAJwsVerifier

[identity\_wasm](../modules/identity_wasm.md).EcDSAJwsVerifier

An implementor of `IJwsVerifier` that can handle the
`EcDSA` algorithm.

## Table of contents

### Constructors

- [constructor](identity_wasm.EcDSAJwsVerifier.md#constructor)

### Methods

- [verify](identity_wasm.EcDSAJwsVerifier.md#verify)

## Constructors

### constructor

• **new EcDSAJwsVerifier**()

Constructs an EcDSAJwsVerifier.

## Methods

### verify

▸ **verify**(`alg`, `signingInput`, `decodedSignature`, `publicKey`): `void`

Verify a JWS signature secured with the `EcDSA` algorithm.
Only the `ES256` and `ES256K` curves are supported for now.

# Warning

This function does not check the `alg` property in the protected header. Callers are expected to assert this
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
