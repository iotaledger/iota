# Class: JwtDomainLinkageValidator

[identity\_wasm](../modules/identity_wasm.md).JwtDomainLinkageValidator

A validator for a Domain Linkage Configuration and Credentials.

## Table of contents

### Constructors

- [constructor](identity_wasm.JwtDomainLinkageValidator.md#constructor)

### Methods

- [validateLinkage](identity_wasm.JwtDomainLinkageValidator.md#validatelinkage)
- [validateCredential](identity_wasm.JwtDomainLinkageValidator.md#validatecredential)

## Constructors

### constructor

• **new JwtDomainLinkageValidator**(`signatureVerifier?`)

Creates a new [JwtDomainLinkageValidator](identity_wasm.JwtDomainLinkageValidator.md). If a `signatureVerifier` is provided it will be used when
verifying decoded JWS signatures, otherwise a default verifier capable of handling the `EdDSA`, `ES256`, `ES256K`
algorithms will be used.

#### Parameters

| Name | Type |
| :------ | :------ |
| `signatureVerifier?` | [`IJwsVerifier`](../interfaces/identity_wasm.IJwsVerifier.md) |

## Methods

### validateLinkage

▸ **validateLinkage**(`issuer`, `configuration`, `domain`, `options`): `void`

Validates the linkage between a domain and a DID.
[DomainLinkageConfiguration](identity_wasm.DomainLinkageConfiguration.md) is validated according to [DID Configuration Resource Verification](https://identity.foundation/.well-known/resources/did-configuration/#did-configuration-resource-verification).

Linkage is valid if no error is thrown.

# Note:
- Only the [JSON Web Token Proof Format](https://identity.foundation/.well-known/resources/did-configuration/#json-web-token-proof-format)
  is supported.
- Only the Credential issued by `issuer` is verified.

# Errors

 - Semantic structure of `configuration` is invalid.
 - `configuration` includes multiple credentials issued by `issuer`.
 - Validation of the matched Domain Linkage Credential fails.

#### Parameters

| Name | Type |
| :------ | :------ |
| `issuer` | `IToCoreDocument` \| [`CoreDocument`](identity_wasm.CoreDocument.md) |
| `configuration` | [`DomainLinkageConfiguration`](identity_wasm.DomainLinkageConfiguration.md) |
| `domain` | `string` |
| `options` | [`JwtCredentialValidationOptions`](identity_wasm.JwtCredentialValidationOptions.md) |

#### Returns

`void`

___

### validateCredential

▸ **validateCredential**(`issuer`, `credentialJwt`, `domain`, `options`): `void`

Validates a [Domain Linkage Credential](https://identity.foundation/.well-known/resources/did-configuration/#domain-linkage-credential).

Error will be thrown in case the validation fails.

#### Parameters

| Name | Type |
| :------ | :------ |
| `issuer` | `IToCoreDocument` \| [`CoreDocument`](identity_wasm.CoreDocument.md) |
| `credentialJwt` | [`Jwt`](identity_wasm.Jwt.md) |
| `domain` | `string` |
| `options` | [`JwtCredentialValidationOptions`](identity_wasm.JwtCredentialValidationOptions.md) |

#### Returns

`void`
