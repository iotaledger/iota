# Class: SdJwtCredentialValidator

[identity\_wasm](../modules/identity_wasm.md).SdJwtCredentialValidator

A type for decoding and validating [Credential](identity_wasm.Credential.md).

## Table of contents

### Constructors

- [constructor](identity_wasm.SdJwtCredentialValidator.md#constructor)

### Methods

- [validateCredential](identity_wasm.SdJwtCredentialValidator.md#validatecredential)
- [verifySignature](identity_wasm.SdJwtCredentialValidator.md#verifysignature)
- [validateKeyBindingJwt](identity_wasm.SdJwtCredentialValidator.md#validatekeybindingjwt)

## Constructors

### constructor

• **new SdJwtCredentialValidator**(`signatureVerifier?`)

Creates a new `SdJwtCredentialValidator`. If a `signatureVerifier` is provided it will be used when
verifying decoded JWS signatures, otherwise a default verifier capable of handling the `EdDSA`, `ES256`, `ES256K`
algorithms will be used.

#### Parameters

| Name | Type |
| :------ | :------ |
| `signatureVerifier?` | [`IJwsVerifier`](../interfaces/identity_wasm.IJwsVerifier.md) |

## Methods

### validateCredential

▸ **validateCredential**(`sd_jwt`, `issuer`, `options`, `fail_fast`): [`DecodedJwtCredential`](identity_wasm.DecodedJwtCredential.md)

Decodes and validates a `Credential` issued as an SD-JWT. A `DecodedJwtCredential` is returned upon success.
The credential is constructed by replacing disclosures following the
[`Selective Disclosure for JWTs (SD-JWT)`](https://www.ietf.org/archive/id/draft-ietf-oauth-selective-disclosure-jwt-07.html) standard.

The following properties are validated according to `options`:
- the issuer's signature on the JWS,
- the expiration date,
- the issuance date,
- the semantic structure.

# Warning
* The key binding JWT is not validated. If needed, it must be validated separately using
`SdJwtValidator::validate_key_binding_jwt`.
* The lack of an error returned from this method is in of itself not enough to conclude that the credential can be
trusted. This section contains more information on additional checks that should be carried out before and after
calling this method.

## The state of the issuer's DID Document
The caller must ensure that `issuer` represents an up-to-date DID Document.

## Properties that are not validated
 There are many properties defined in [The Verifiable Credentials Data Model](https://www.w3.org/TR/vc-data-model/) that are **not** validated, such as:
`proof`, `credentialStatus`, `type`, `credentialSchema`, `refreshService` **and more**.
These should be manually checked after validation, according to your requirements.

# Errors
An error is returned whenever a validated condition is not satisfied.

#### Parameters

| Name | Type |
| :------ | :------ |
| `sd_jwt` | [`SdJwt`](identity_wasm.SdJwt.md) |
| `issuer` | `IToCoreDocument` \| [`CoreDocument`](identity_wasm.CoreDocument.md) |
| `options` | [`JwtCredentialValidationOptions`](identity_wasm.JwtCredentialValidationOptions.md) |
| `fail_fast` | [`FailFast`](../enums/identity_wasm.FailFast.md) |

#### Returns

[`DecodedJwtCredential`](identity_wasm.DecodedJwtCredential.md)

___

### verifySignature

▸ **verifySignature**(`credential`, `trustedIssuers`, `options`): [`DecodedJwtCredential`](identity_wasm.DecodedJwtCredential.md)

Decode and verify the JWS signature of a `Credential` issued as an SD-JWT using the DID Document of a trusted
issuer and replaces the disclosures.

A `DecodedJwtCredential` is returned upon success.

# Warning
The caller must ensure that the DID Documents of the trusted issuers are up-to-date.

## Proofs
 Only the JWS signature is verified. If the `Credential` contains a `proof` property this will not be verified
by this method.

# Errors
* If the issuer' URL cannot be parsed.
* If Signature verification fails.
* If SD decoding fails.

#### Parameters

| Name | Type |
| :------ | :------ |
| `credential` | [`SdJwt`](identity_wasm.SdJwt.md) |
| `trustedIssuers` | (`IToCoreDocument` \| [`CoreDocument`](identity_wasm.CoreDocument.md))[] |
| `options` | [`JwsVerificationOptions`](identity_wasm.JwsVerificationOptions.md) |

#### Returns

[`DecodedJwtCredential`](identity_wasm.DecodedJwtCredential.md)

___

### validateKeyBindingJwt

▸ **validateKeyBindingJwt**(`sdJwt`, `holder`, `options`): [`KeyBindingJwtClaims`](identity_wasm.KeyBindingJwtClaims.md)

Validates a Key Binding JWT (KB-JWT) according to `https://www.ietf.org/archive/id/draft-ietf-oauth-selective-disclosure-jwt-07.html#name-key-binding-jwt`.
The Validation process includes:
  * Signature validation using public key materials defined in the `holder` document.
  * `typ` value in KB-JWT header.
  * `sd_hash` claim value in the KB-JWT claim.
  * Optional `nonce`, `aud` and issuance date validation.

#### Parameters

| Name | Type |
| :------ | :------ |
| `sdJwt` | [`SdJwt`](identity_wasm.SdJwt.md) |
| `holder` | `IToCoreDocument` \| [`CoreDocument`](identity_wasm.CoreDocument.md) |
| `options` | [`KeyBindingJWTValidationOptions`](identity_wasm.KeyBindingJWTValidationOptions.md) |

#### Returns

[`KeyBindingJwtClaims`](identity_wasm.KeyBindingJwtClaims.md)
