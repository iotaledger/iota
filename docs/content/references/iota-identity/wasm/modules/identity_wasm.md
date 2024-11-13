# Module: identity\_wasm

## Table of contents

### Enumerations

- [FailFast](../enums/identity_wasm.FailFast.md)
- [SubjectHolderRelationship](../enums/identity_wasm.SubjectHolderRelationship.md)
- [MethodRelationship](../enums/identity_wasm.MethodRelationship.md)
- [StateMetadataEncoding](../enums/identity_wasm.StateMetadataEncoding.md)
- [StatusCheck](../enums/identity_wasm.StatusCheck.md)
- [PresentationProofAlgorithm](../enums/identity_wasm.PresentationProofAlgorithm.md)
- [CredentialStatus](../enums/identity_wasm.CredentialStatus.md)
- [StatusPurpose](../enums/identity_wasm.StatusPurpose.md)
- [PayloadType](../enums/identity_wasm.PayloadType.md)
- [SerializationType](../enums/identity_wasm.SerializationType.md)
- [ProofAlgorithm](../enums/identity_wasm.ProofAlgorithm.md)

### Classes

- [CoreDID](../classes/identity_wasm.CoreDID.md)
- [CoreDocument](../classes/identity_wasm.CoreDocument.md)
- [Credential](../classes/identity_wasm.Credential.md)
- [CustomMethodData](../classes/identity_wasm.CustomMethodData.md)
- [DIDJwk](../classes/identity_wasm.DIDJwk.md)
- [DIDUrl](../classes/identity_wasm.DIDUrl.md)
- [DecodedJptCredential](../classes/identity_wasm.DecodedJptCredential.md)
- [DecodedJptPresentation](../classes/identity_wasm.DecodedJptPresentation.md)
- [DecodedJws](../classes/identity_wasm.DecodedJws.md)
- [DecodedJwtCredential](../classes/identity_wasm.DecodedJwtCredential.md)
- [DecodedJwtPresentation](../classes/identity_wasm.DecodedJwtPresentation.md)
- [Disclosure](../classes/identity_wasm.Disclosure.md)
- [DomainLinkageConfiguration](../classes/identity_wasm.DomainLinkageConfiguration.md)
- [Duration](../classes/identity_wasm.Duration.md)
- [EcDSAJwsVerifier](../classes/identity_wasm.EcDSAJwsVerifier.md)
- [EdDSAJwsVerifier](../classes/identity_wasm.EdDSAJwsVerifier.md)
- [IotaDID](../classes/identity_wasm.IotaDID.md)
- [IotaDocument](../classes/identity_wasm.IotaDocument.md)
- [IotaDocumentMetadata](../classes/identity_wasm.IotaDocumentMetadata.md)
- [IotaIdentityClientExt](../classes/identity_wasm.IotaIdentityClientExt.md)
- [IssuerProtectedHeader](../classes/identity_wasm.IssuerProtectedHeader.md)
- [Jpt](../classes/identity_wasm.Jpt.md)
- [JptCredentialValidationOptions](../classes/identity_wasm.JptCredentialValidationOptions.md)
- [JptCredentialValidator](../classes/identity_wasm.JptCredentialValidator.md)
- [JptCredentialValidatorUtils](../classes/identity_wasm.JptCredentialValidatorUtils.md)
- [JptPresentationValidationOptions](../classes/identity_wasm.JptPresentationValidationOptions.md)
- [JptPresentationValidator](../classes/identity_wasm.JptPresentationValidator.md)
- [JptPresentationValidatorUtils](../classes/identity_wasm.JptPresentationValidatorUtils.md)
- [Jwk](../classes/identity_wasm.Jwk.md)
- [JwkGenOutput](../classes/identity_wasm.JwkGenOutput.md)
- [JwpCredentialOptions](../classes/identity_wasm.JwpCredentialOptions.md)
- [JwpIssued](../classes/identity_wasm.JwpIssued.md)
- [JwpPresentationOptions](../classes/identity_wasm.JwpPresentationOptions.md)
- [JwpVerificationOptions](../classes/identity_wasm.JwpVerificationOptions.md)
- [Jws](../classes/identity_wasm.Jws.md)
- [JwsHeader](../classes/identity_wasm.JwsHeader.md)
- [JwsSignatureOptions](../classes/identity_wasm.JwsSignatureOptions.md)
- [JwsVerificationOptions](../classes/identity_wasm.JwsVerificationOptions.md)
- [Jwt](../classes/identity_wasm.Jwt.md)
- [JwtCredentialValidationOptions](../classes/identity_wasm.JwtCredentialValidationOptions.md)
- [JwtCredentialValidator](../classes/identity_wasm.JwtCredentialValidator.md)
- [JwtDomainLinkageValidator](../classes/identity_wasm.JwtDomainLinkageValidator.md)
- [JwtPresentationOptions](../classes/identity_wasm.JwtPresentationOptions.md)
- [JwtPresentationValidationOptions](../classes/identity_wasm.JwtPresentationValidationOptions.md)
- [JwtPresentationValidator](../classes/identity_wasm.JwtPresentationValidator.md)
- [KeyBindingJWTValidationOptions](../classes/identity_wasm.KeyBindingJWTValidationOptions.md)
- [KeyBindingJwtClaims](../classes/identity_wasm.KeyBindingJwtClaims.md)
- [LinkedDomainService](../classes/identity_wasm.LinkedDomainService.md)
- [LinkedVerifiablePresentationService](../classes/identity_wasm.LinkedVerifiablePresentationService.md)
- [MethodData](../classes/identity_wasm.MethodData.md)
- [MethodDigest](../classes/identity_wasm.MethodDigest.md)
- [MethodScope](../classes/identity_wasm.MethodScope.md)
- [MethodType](../classes/identity_wasm.MethodType.md)
- [PayloadEntry](../classes/identity_wasm.PayloadEntry.md)
- [Payloads](../classes/identity_wasm.Payloads.md)
- [Presentation](../classes/identity_wasm.Presentation.md)
- [PresentationProtectedHeader](../classes/identity_wasm.PresentationProtectedHeader.md)
- [Proof](../classes/identity_wasm.Proof.md)
- [ProofUpdateCtx](../classes/identity_wasm.ProofUpdateCtx.md)
- [Resolver](../classes/identity_wasm.Resolver.md)
- [RevocationBitmap](../classes/identity_wasm.RevocationBitmap.md)
- [RevocationTimeframeStatus](../classes/identity_wasm.RevocationTimeframeStatus.md)
- [SdJwt](../classes/identity_wasm.SdJwt.md)
- [SdJwtCredentialValidator](../classes/identity_wasm.SdJwtCredentialValidator.md)
- [SdObjectDecoder](../classes/identity_wasm.SdObjectDecoder.md)
- [SdObjectEncoder](../classes/identity_wasm.SdObjectEncoder.md)
- [SelectiveDisclosurePresentation](../classes/identity_wasm.SelectiveDisclosurePresentation.md)
- [Service](../classes/identity_wasm.Service.md)
- [StatusList2021](../classes/identity_wasm.StatusList2021.md)
- [StatusList2021Credential](../classes/identity_wasm.StatusList2021Credential.md)
- [StatusList2021CredentialBuilder](../classes/identity_wasm.StatusList2021CredentialBuilder.md)
- [StatusList2021Entry](../classes/identity_wasm.StatusList2021Entry.md)
- [Storage](../classes/identity_wasm.Storage.md)
- [Timestamp](../classes/identity_wasm.Timestamp.md)
- [UnknownCredential](../classes/identity_wasm.UnknownCredential.md)
- [VerificationMethod](../classes/identity_wasm.VerificationMethod.md)

### Interfaces

- [IJptCredentialValidationOptions](../interfaces/identity_wasm.IJptCredentialValidationOptions.md)
- [IJwpVerificationOptions](../interfaces/identity_wasm.IJwpVerificationOptions.md)
- [IJptPresentationValidationOptions](../interfaces/identity_wasm.IJptPresentationValidationOptions.md)
- [IPresentation](../interfaces/identity_wasm.IPresentation.md)
- [JwkStorage](../interfaces/identity_wasm.JwkStorage.md)
- [KeyIdStorage](../interfaces/identity_wasm.KeyIdStorage.md)
- [IDomainLinkageCredential](../interfaces/identity_wasm.IDomainLinkageCredential.md)
- [IJwsVerificationOptions](../interfaces/identity_wasm.IJwsVerificationOptions.md)
- [IJwkEc](../interfaces/identity_wasm.IJwkEc.md)
- [IJwkRsa](../interfaces/identity_wasm.IJwkRsa.md)
- [IJwkOkp](../interfaces/identity_wasm.IJwkOkp.md)
- [IJwkOct](../interfaces/identity_wasm.IJwkOct.md)
- [IJwk](../interfaces/identity_wasm.IJwk.md)
- [JwkParamsEc](../interfaces/identity_wasm.JwkParamsEc.md)
- [JwkParamsOkp](../interfaces/identity_wasm.JwkParamsOkp.md)
- [JwkParamsRsa](../interfaces/identity_wasm.JwkParamsRsa.md)
- [JwkParamsRsaPrime](../interfaces/identity_wasm.JwkParamsRsaPrime.md)
- [JwkParamsOct](../interfaces/identity_wasm.JwkParamsOct.md)
- [JptClaims](../interfaces/identity_wasm.JptClaims.md)
- [IJwtPresentationOptions](../interfaces/identity_wasm.IJwtPresentationOptions.md)
- [IKeyBindingJWTValidationOptions](../interfaces/identity_wasm.IKeyBindingJWTValidationOptions.md)
- [IJwtCredentialValidationOptions](../interfaces/identity_wasm.IJwtCredentialValidationOptions.md)
- [IJwtPresentationValidationOptions](../interfaces/identity_wasm.IJwtPresentationValidationOptions.md)
- [ILinkedDomainService](../interfaces/identity_wasm.ILinkedDomainService.md)
- [ILinkedVerifiablePresentationService](../interfaces/identity_wasm.ILinkedVerifiablePresentationService.md)
- [IService](../interfaces/identity_wasm.IService.md)
- [IJwsSignatureOptions](../interfaces/identity_wasm.IJwsSignatureOptions.md)
- [Evidence](../interfaces/identity_wasm.Evidence.md)
- [Issuer](../interfaces/identity_wasm.Issuer.md)
- [Policy](../interfaces/identity_wasm.Policy.md)
- [RefreshService](../interfaces/identity_wasm.RefreshService.md)
- [Schema](../interfaces/identity_wasm.Schema.md)
- [Status](../interfaces/identity_wasm.Status.md)
- [Subject](../interfaces/identity_wasm.Subject.md)
- [ICredential](../interfaces/identity_wasm.ICredential.md)
- [IIotaIdentityClient](../interfaces/identity_wasm.IIotaIdentityClient.md)
- [IJwsVerifier](../interfaces/identity_wasm.IJwsVerifier.md)

### Type Aliases

- [ResolverConfig](identity_wasm.md#resolverconfig)

### Functions

- [verifyEd25519](identity_wasm.md#verifyed25519)
- [encodeB64](identity_wasm.md#encodeb64)
- [decodeB64](identity_wasm.md#decodeb64)
- [start](identity_wasm.md#start)

## Type Aliases

### ResolverConfig

Ƭ **ResolverConfig**: `Object`

Configurations for the [Resolver](../classes/identity_wasm.Resolver.md).

#### Type declaration

| Name | Type | Description |
| :------ | :------ | :------ |
| `client?` | [`IIotaIdentityClient`](../interfaces/identity_wasm.IIotaIdentityClient.md) | Client for resolving DIDs of the iota method. |
| `handlers?` | `Map`\<`string`, (`did`: `string`) => `Promise`\<[`CoreDocument`](../classes/identity_wasm.CoreDocument.md) \| `IToCoreDocument`\>\> | Handlers for resolving DIDs from arbitrary DID methods. The keys to the map are expected to match the method name and the values are asynchronous functions returning DID documents. Note that if a `client` is given the key "iota" may NOT be present in this map. |

## Functions

### verifyEd25519

▸ **verifyEd25519**(`alg`, `signingInput`, `decodedSignature`, `publicKey`): `void`

Verify a JWS signature secured with the `EdDSA` algorithm and curve `Ed25519`.

This function is useful when one is composing a `IJwsVerifier` that delegates
`EdDSA` verification with curve `Ed25519` to this function.

# Warning

This function does not check whether `alg = EdDSA` in the protected header. Callers are expected to assert this
prior to calling the function.

#### Parameters

| Name | Type |
| :------ | :------ |
| `alg` | [`JwsAlgorithm`](../enums/jose_jws_algorithm.JwsAlgorithm.md) |
| `signingInput` | `Uint8Array` |
| `decodedSignature` | `Uint8Array` |
| `publicKey` | [`Jwk`](../classes/identity_wasm.Jwk.md) |

#### Returns

`void`

___

### encodeB64

▸ **encodeB64**(`data`): `string`

Encode the given bytes in url-safe base64.

#### Parameters

| Name | Type |
| :------ | :------ |
| `data` | `Uint8Array` |

#### Returns

`string`

___

### decodeB64

▸ **decodeB64**(`data`): `Uint8Array`

Decode the given url-safe base64-encoded slice into its raw bytes.

#### Parameters

| Name | Type |
| :------ | :------ |
| `data` | `Uint8Array` |

#### Returns

`Uint8Array`

___

### start

▸ **start**(): `void`

Initializes the console error panic hook for better error messages

#### Returns

`void`
