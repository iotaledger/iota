# Interface: IJwsVerifier

[identity\_wasm](../modules/identity_wasm.md).IJwsVerifier

Interface for cryptographically verifying a JWS signature. 

Implementers are expected to provide a procedure for step 8 of [RFC 7515 section 5.2](https://www.rfc-editor.org/rfc/rfc7515#section-5.2) for 
the JWS signature algorithms they want to support.

## Table of contents

### Properties

- [verify](identity_wasm.IJwsVerifier.md#verify)

## Properties

### verify

• **verify**: (`alg`: [`JwsAlgorithm`](../enums/jose_jws_algorithm.JwsAlgorithm.md), `signingInput`: `Uint8Array`, `decodedSignature`: `Uint8Array`, `publicKey`: [`Jwk`](../classes/identity_wasm.Jwk.md)) => `void`

#### Type declaration

▸ (`alg`, `signingInput`, `decodedSignature`, `publicKey`): `void`

Validate the `decodedSignature` against the `signingInput` in the manner defined by `alg` using the `publicKey`.

 See [RFC 7515: section 5.2 part 8.](https://www.rfc-editor.org/rfc/rfc7515#section-5.2) and
 [RFC 7797 section 3](https://www.rfc-editor.org/rfc/rfc7797#section-3).

### Failures
Upon verification failure an error must be thrown.

##### Parameters

| Name | Type |
| :------ | :------ |
| `alg` | [`JwsAlgorithm`](../enums/jose_jws_algorithm.JwsAlgorithm.md) |
| `signingInput` | `Uint8Array` |
| `decodedSignature` | `Uint8Array` |
| `publicKey` | [`Jwk`](../classes/identity_wasm.Jwk.md) |

##### Returns

`void`
