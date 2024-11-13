# Class: JwtCredentialValidator

[identity\_wasm](../modules/identity_wasm.md).JwtCredentialValidator

A type for decoding and validating [Credential](identity_wasm.Credential.md).

## Table of contents

### Constructors

- [constructor](identity_wasm.JwtCredentialValidator.md#constructor)

### Methods

- [validate](identity_wasm.JwtCredentialValidator.md#validate)
- [verifySignature](identity_wasm.JwtCredentialValidator.md#verifysignature)
- [checkExpiresOnOrAfter](identity_wasm.JwtCredentialValidator.md#checkexpiresonorafter)
- [checkIssuedOnOrBefore](identity_wasm.JwtCredentialValidator.md#checkissuedonorbefore)
- [checkSubjectHolderRelationship](identity_wasm.JwtCredentialValidator.md#checksubjectholderrelationship)
- [checkStatus](identity_wasm.JwtCredentialValidator.md#checkstatus)
- [checkStatusWithStatusList2021](identity_wasm.JwtCredentialValidator.md#checkstatuswithstatuslist2021)
- [extractIssuer](identity_wasm.JwtCredentialValidator.md#extractissuer)
- [extractIssuerFromJwt](identity_wasm.JwtCredentialValidator.md#extractissuerfromjwt)

## Constructors

### constructor

• **new JwtCredentialValidator**(`signatureVerifier?`)

Creates a new [JwtCredentialValidator](identity_wasm.JwtCredentialValidator.md). If a `signatureVerifier` is provided it will be used when
verifying decoded JWS signatures, otherwise a default verifier capable of handling the `EdDSA`, `ES256`, `ES256K`
algorithms will be used.

#### Parameters

| Name | Type |
| :------ | :------ |
| `signatureVerifier?` | [`IJwsVerifier`](../interfaces/identity_wasm.IJwsVerifier.md) |

## Methods

### validate

▸ **validate**(`credential_jwt`, `issuer`, `options`, `fail_fast`): [`DecodedJwtCredential`](identity_wasm.DecodedJwtCredential.md)

Decodes and validates a [Credential](identity_wasm.Credential.md) issued as a JWS. A [DecodedJwtCredential](identity_wasm.DecodedJwtCredential.md) is returned upon
success.

The following properties are validated according to `options`:
- the issuer's signature on the JWS,
- the expiration date,
- the issuance date,
- the semantic structure.

# Warning
The lack of an error returned from this method is in of itself not enough to conclude that the credential can be
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
| `credential_jwt` | [`Jwt`](identity_wasm.Jwt.md) |
| `issuer` | `IToCoreDocument` \| [`CoreDocument`](identity_wasm.CoreDocument.md) |
| `options` | [`JwtCredentialValidationOptions`](identity_wasm.JwtCredentialValidationOptions.md) |
| `fail_fast` | [`FailFast`](../enums/identity_wasm.FailFast.md) |

#### Returns

[`DecodedJwtCredential`](identity_wasm.DecodedJwtCredential.md)

___

### verifySignature

▸ **verifySignature**(`credential`, `trustedIssuers`, `options`): [`DecodedJwtCredential`](identity_wasm.DecodedJwtCredential.md)

Decode and verify the JWS signature of a [Credential](identity_wasm.Credential.md) issued as a JWT using the DID Document of a trusted
issuer.

A [DecodedJwtCredential](identity_wasm.DecodedJwtCredential.md) is returned upon success.

# Warning
The caller must ensure that the DID Documents of the trusted issuers are up-to-date.

## Proofs
 Only the JWS signature is verified. If the [Credential](identity_wasm.Credential.md) contains a `proof` property this will not be
verified by this method.

# Errors
This method immediately returns an error if
the credential issuer' url cannot be parsed to a DID belonging to one of the trusted issuers. Otherwise an attempt
to verify the credential's signature will be made and an error is returned upon failure.

#### Parameters

| Name | Type |
| :------ | :------ |
| `credential` | [`Jwt`](identity_wasm.Jwt.md) |
| `trustedIssuers` | (`IToCoreDocument` \| [`CoreDocument`](identity_wasm.CoreDocument.md))[] |
| `options` | [`JwsVerificationOptions`](identity_wasm.JwsVerificationOptions.md) |

#### Returns

[`DecodedJwtCredential`](identity_wasm.DecodedJwtCredential.md)

___

### checkExpiresOnOrAfter

▸ `Static` **checkExpiresOnOrAfter**(`credential`, `timestamp`): `void`

Validate that the credential expires on or after the specified timestamp.

#### Parameters

| Name | Type |
| :------ | :------ |
| `credential` | [`Credential`](identity_wasm.Credential.md) |
| `timestamp` | [`Timestamp`](identity_wasm.Timestamp.md) |

#### Returns

`void`

___

### checkIssuedOnOrBefore

▸ `Static` **checkIssuedOnOrBefore**(`credential`, `timestamp`): `void`

Validate that the credential is issued on or before the specified timestamp.

#### Parameters

| Name | Type |
| :------ | :------ |
| `credential` | [`Credential`](identity_wasm.Credential.md) |
| `timestamp` | [`Timestamp`](identity_wasm.Timestamp.md) |

#### Returns

`void`

___

### checkSubjectHolderRelationship

▸ `Static` **checkSubjectHolderRelationship**(`credential`, `holder`, `relationship`): `void`

Validate that the relationship between the `holder` and the credential subjects is in accordance with
`relationship`. The `holder` parameter is expected to be the URL of the holder.

#### Parameters

| Name | Type |
| :------ | :------ |
| `credential` | [`Credential`](identity_wasm.Credential.md) |
| `holder` | `string` |
| `relationship` | [`SubjectHolderRelationship`](../enums/identity_wasm.SubjectHolderRelationship.md) |

#### Returns

`void`

___

### checkStatus

▸ `Static` **checkStatus**(`credential`, `trustedIssuers`, `statusCheck`): `void`

Checks whether the credential status has been revoked.

Only supports `RevocationBitmap2022`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `credential` | [`Credential`](identity_wasm.Credential.md) |
| `trustedIssuers` | (`IToCoreDocument` \| [`CoreDocument`](identity_wasm.CoreDocument.md))[] |
| `statusCheck` | [`StatusCheck`](../enums/identity_wasm.StatusCheck.md) |

#### Returns

`void`

___

### checkStatusWithStatusList2021

▸ `Static` **checkStatusWithStatusList2021**(`credential`, `status_list`, `status_check`): `void`

Checks wheter the credential status has been revoked using `StatusList2021`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `credential` | [`Credential`](identity_wasm.Credential.md) |
| `status_list` | [`StatusList2021Credential`](identity_wasm.StatusList2021Credential.md) |
| `status_check` | [`StatusCheck`](../enums/identity_wasm.StatusCheck.md) |

#### Returns

`void`

___

### extractIssuer

▸ `Static` **extractIssuer**(`credential`): [`CoreDID`](identity_wasm.CoreDID.md)

Utility for extracting the issuer field of a [Credential](identity_wasm.Credential.md) as a DID.

### Errors

Fails if the issuer field is not a valid DID.

#### Parameters

| Name | Type |
| :------ | :------ |
| `credential` | [`Credential`](identity_wasm.Credential.md) |

#### Returns

[`CoreDID`](identity_wasm.CoreDID.md)

___

### extractIssuerFromJwt

▸ `Static` **extractIssuerFromJwt**(`credential`): [`CoreDID`](identity_wasm.CoreDID.md)

Utility for extracting the issuer field of a credential in JWT representation as DID.

# Errors

If the JWT decoding fails or the issuer field is not a valid DID.

#### Parameters

| Name | Type |
| :------ | :------ |
| `credential` | [`Jwt`](identity_wasm.Jwt.md) |

#### Returns

[`CoreDID`](identity_wasm.CoreDID.md)
