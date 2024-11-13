# Class: CoreDocument

[identity\_wasm](../modules/identity_wasm.md).CoreDocument

A method-agnostic DID Document.

Note: All methods that involve reading from this class may potentially raise an error
if the object is being concurrently modified.

## Table of contents

### Constructors

- [constructor](identity_wasm.CoreDocument.md#constructor)

### Methods

- [toJSON](identity_wasm.CoreDocument.md#tojson)
- [toString](identity_wasm.CoreDocument.md#tostring)
- [id](identity_wasm.CoreDocument.md#id)
- [setId](identity_wasm.CoreDocument.md#setid)
- [controller](identity_wasm.CoreDocument.md#controller)
- [setController](identity_wasm.CoreDocument.md#setcontroller)
- [alsoKnownAs](identity_wasm.CoreDocument.md#alsoknownas)
- [setAlsoKnownAs](identity_wasm.CoreDocument.md#setalsoknownas)
- [verificationMethod](identity_wasm.CoreDocument.md#verificationmethod)
- [authentication](identity_wasm.CoreDocument.md#authentication)
- [assertionMethod](identity_wasm.CoreDocument.md#assertionmethod)
- [keyAgreement](identity_wasm.CoreDocument.md#keyagreement)
- [capabilityDelegation](identity_wasm.CoreDocument.md#capabilitydelegation)
- [capabilityInvocation](identity_wasm.CoreDocument.md#capabilityinvocation)
- [properties](identity_wasm.CoreDocument.md#properties)
- [setPropertyUnchecked](identity_wasm.CoreDocument.md#setpropertyunchecked)
- [service](identity_wasm.CoreDocument.md#service)
- [insertService](identity_wasm.CoreDocument.md#insertservice)
- [removeService](identity_wasm.CoreDocument.md#removeservice)
- [resolveService](identity_wasm.CoreDocument.md#resolveservice)
- [methods](identity_wasm.CoreDocument.md#methods)
- [verificationRelationships](identity_wasm.CoreDocument.md#verificationrelationships)
- [insertMethod](identity_wasm.CoreDocument.md#insertmethod)
- [removeMethod](identity_wasm.CoreDocument.md#removemethod)
- [resolveMethod](identity_wasm.CoreDocument.md#resolvemethod)
- [attachMethodRelationship](identity_wasm.CoreDocument.md#attachmethodrelationship)
- [detachMethodRelationship](identity_wasm.CoreDocument.md#detachmethodrelationship)
- [verifyJws](identity_wasm.CoreDocument.md#verifyjws)
- [revokeCredentials](identity_wasm.CoreDocument.md#revokecredentials)
- [unrevokeCredentials](identity_wasm.CoreDocument.md#unrevokecredentials)
- [clone](identity_wasm.CoreDocument.md#clone)
- [\_shallowCloneInternal](identity_wasm.CoreDocument.md#_shallowcloneinternal)
- [\_strongCountInternal](identity_wasm.CoreDocument.md#_strongcountinternal)
- [fromJSON](identity_wasm.CoreDocument.md#fromjson)
- [generateMethod](identity_wasm.CoreDocument.md#generatemethod)
- [purgeMethod](identity_wasm.CoreDocument.md#purgemethod)
- [createJws](identity_wasm.CoreDocument.md#createjws)
- [createCredentialJwt](identity_wasm.CoreDocument.md#createcredentialjwt)
- [createPresentationJwt](identity_wasm.CoreDocument.md#createpresentationjwt)
- [expandDIDJwk](identity_wasm.CoreDocument.md#expanddidjwk)

## Constructors

### constructor

• **new CoreDocument**(`values`)

Creates a new [CoreDocument](identity_wasm.CoreDocument.md) with the given properties.

#### Parameters

| Name | Type |
| :------ | :------ |
| `values` | `ICoreDocument` |

## Methods

### toJSON

▸ **toJSON**(): `Object`

* Return copy of self without private attributes.

#### Returns

`Object`

▸ **toJSON**(): `any`

Serializes to a plain JS representation.

#### Returns

`any`

___

### toString

▸ **toString**(): `string`

Return stringified version of self.

#### Returns

`string`

___

### id

▸ **id**(): [`CoreDID`](identity_wasm.CoreDID.md)

Returns a copy of the DID Document `id`.

#### Returns

[`CoreDID`](identity_wasm.CoreDID.md)

___

### setId

▸ **setId**(`id`): `void`

Sets the DID of the document.

### Warning

Changing the identifier can drastically alter the results of
`resolve_method`, `resolve_service` and the related
[DID URL dereferencing](https://w3c-ccg.github.io/did-resolution/#dereferencing) algorithm.

#### Parameters

| Name | Type |
| :------ | :------ |
| `id` | [`CoreDID`](identity_wasm.CoreDID.md) |

#### Returns

`void`

___

### controller

▸ **controller**(): [`CoreDID`](identity_wasm.CoreDID.md)[]

Returns a copy of the document controllers.

#### Returns

[`CoreDID`](identity_wasm.CoreDID.md)[]

___

### setController

▸ **setController**(`controllers`): `void`

Sets the controllers of the DID Document.

Note: Duplicates will be ignored.
Use `null` to remove all controllers.

#### Parameters

| Name | Type |
| :------ | :------ |
| `controllers` | ``null`` \| [`CoreDID`](identity_wasm.CoreDID.md) \| [`CoreDID`](identity_wasm.CoreDID.md)[] |

#### Returns

`void`

___

### alsoKnownAs

▸ **alsoKnownAs**(): `string`[]

Returns a copy of the document's `alsoKnownAs` set.

#### Returns

`string`[]

___

### setAlsoKnownAs

▸ **setAlsoKnownAs**(`urls`): `void`

Sets the `alsoKnownAs` property in the DID document.

#### Parameters

| Name | Type |
| :------ | :------ |
| `urls` | ``null`` \| `string` \| `string`[] |

#### Returns

`void`

___

### verificationMethod

▸ **verificationMethod**(): [`VerificationMethod`](identity_wasm.VerificationMethod.md)[]

Returns a copy of the document's `verificationMethod` set.

#### Returns

[`VerificationMethod`](identity_wasm.VerificationMethod.md)[]

___

### authentication

▸ **authentication**(): ([`DIDUrl`](identity_wasm.DIDUrl.md) \| [`VerificationMethod`](identity_wasm.VerificationMethod.md))[]

Returns a copy of the document's `authentication` set.

#### Returns

([`DIDUrl`](identity_wasm.DIDUrl.md) \| [`VerificationMethod`](identity_wasm.VerificationMethod.md))[]

___

### assertionMethod

▸ **assertionMethod**(): ([`DIDUrl`](identity_wasm.DIDUrl.md) \| [`VerificationMethod`](identity_wasm.VerificationMethod.md))[]

Returns a copy of the document's `assertionMethod` set.

#### Returns

([`DIDUrl`](identity_wasm.DIDUrl.md) \| [`VerificationMethod`](identity_wasm.VerificationMethod.md))[]

___

### keyAgreement

▸ **keyAgreement**(): ([`DIDUrl`](identity_wasm.DIDUrl.md) \| [`VerificationMethod`](identity_wasm.VerificationMethod.md))[]

Returns a copy of the document's `keyAgreement` set.

#### Returns

([`DIDUrl`](identity_wasm.DIDUrl.md) \| [`VerificationMethod`](identity_wasm.VerificationMethod.md))[]

___

### capabilityDelegation

▸ **capabilityDelegation**(): ([`DIDUrl`](identity_wasm.DIDUrl.md) \| [`VerificationMethod`](identity_wasm.VerificationMethod.md))[]

Returns a copy of the document's `capabilityDelegation` set.

#### Returns

([`DIDUrl`](identity_wasm.DIDUrl.md) \| [`VerificationMethod`](identity_wasm.VerificationMethod.md))[]

___

### capabilityInvocation

▸ **capabilityInvocation**(): ([`DIDUrl`](identity_wasm.DIDUrl.md) \| [`VerificationMethod`](identity_wasm.VerificationMethod.md))[]

Returns a copy of the document's `capabilityInvocation` set.

#### Returns

([`DIDUrl`](identity_wasm.DIDUrl.md) \| [`VerificationMethod`](identity_wasm.VerificationMethod.md))[]

___

### properties

▸ **properties**(): `Map`\<`string`, `any`\>

Returns a copy of the custom DID Document properties.

#### Returns

`Map`\<`string`, `any`\>

___

### setPropertyUnchecked

▸ **setPropertyUnchecked**(`key`, `value`): `void`

Sets a custom property in the DID Document.
If the value is set to `null`, the custom property will be removed.

### WARNING

This method can overwrite existing properties like `id` and result in an invalid document.

#### Parameters

| Name | Type |
| :------ | :------ |
| `key` | `string` |
| `value` | `any` |

#### Returns

`void`

___

### service

▸ **service**(): [`Service`](identity_wasm.Service.md)[]

Returns a set of all [Service](identity_wasm.Service.md) in the document.

#### Returns

[`Service`](identity_wasm.Service.md)[]

___

### insertService

▸ **insertService**(`service`): `void`

Add a new [Service](identity_wasm.Service.md) to the document.

Errors if there already exists a service or verification method with the same id.

#### Parameters

| Name | Type |
| :------ | :------ |
| `service` | [`Service`](identity_wasm.Service.md) |

#### Returns

`void`

___

### removeService

▸ **removeService**(`didUrl`): `undefined` \| [`Service`](identity_wasm.Service.md)

Remove a [Service](identity_wasm.Service.md) identified by the given [DIDUrl](identity_wasm.DIDUrl.md) from the document.

Returns `true` if the service was removed.

#### Parameters

| Name | Type |
| :------ | :------ |
| `didUrl` | [`DIDUrl`](identity_wasm.DIDUrl.md) |

#### Returns

`undefined` \| [`Service`](identity_wasm.Service.md)

___

### resolveService

▸ **resolveService**(`query`): `undefined` \| [`Service`](identity_wasm.Service.md)

Returns the first [Service](identity_wasm.Service.md) with an `id` property matching the provided `query`,
if present.

#### Parameters

| Name | Type |
| :------ | :------ |
| `query` | `string` \| [`DIDUrl`](identity_wasm.DIDUrl.md) |

#### Returns

`undefined` \| [`Service`](identity_wasm.Service.md)

___

### methods

▸ **methods**(`scope?`): [`VerificationMethod`](identity_wasm.VerificationMethod.md)[]

Returns a list of all [VerificationMethod](identity_wasm.VerificationMethod.md) in the DID Document,
whose verification relationship matches `scope`.

If `scope` is not set, a list over the **embedded** methods is returned.

#### Parameters

| Name | Type |
| :------ | :------ |
| `scope?` | [`MethodScope`](identity_wasm.MethodScope.md) |

#### Returns

[`VerificationMethod`](identity_wasm.VerificationMethod.md)[]

___

### verificationRelationships

▸ **verificationRelationships**(): ([`DIDUrl`](identity_wasm.DIDUrl.md) \| [`VerificationMethod`](identity_wasm.VerificationMethod.md))[]

Returns an array of all verification relationships.

#### Returns

([`DIDUrl`](identity_wasm.DIDUrl.md) \| [`VerificationMethod`](identity_wasm.VerificationMethod.md))[]

___

### insertMethod

▸ **insertMethod**(`method`, `scope`): `void`

Adds a new `method` to the document in the given `scope`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `method` | [`VerificationMethod`](identity_wasm.VerificationMethod.md) |
| `scope` | [`MethodScope`](identity_wasm.MethodScope.md) |

#### Returns

`void`

___

### removeMethod

▸ **removeMethod**(`did`): `undefined` \| [`VerificationMethod`](identity_wasm.VerificationMethod.md)

Removes all references to the specified Verification Method.

#### Parameters

| Name | Type |
| :------ | :------ |
| `did` | [`DIDUrl`](identity_wasm.DIDUrl.md) |

#### Returns

`undefined` \| [`VerificationMethod`](identity_wasm.VerificationMethod.md)

___

### resolveMethod

▸ **resolveMethod**(`query`, `scope?`): `undefined` \| [`VerificationMethod`](identity_wasm.VerificationMethod.md)

Returns a copy of the first verification method with an `id` property
matching the provided `query` and the verification relationship
specified by `scope`, if present.

#### Parameters

| Name | Type |
| :------ | :------ |
| `query` | `string` \| [`DIDUrl`](identity_wasm.DIDUrl.md) |
| `scope?` | [`MethodScope`](identity_wasm.MethodScope.md) |

#### Returns

`undefined` \| [`VerificationMethod`](identity_wasm.VerificationMethod.md)

___

### attachMethodRelationship

▸ **attachMethodRelationship**(`didUrl`, `relationship`): `boolean`

Attaches the relationship to the given method, if the method exists.

Note: The method needs to be in the set of verification methods,
so it cannot be an embedded one.

#### Parameters

| Name | Type |
| :------ | :------ |
| `didUrl` | [`DIDUrl`](identity_wasm.DIDUrl.md) |
| `relationship` | [`MethodRelationship`](../enums/identity_wasm.MethodRelationship.md) |

#### Returns

`boolean`

___

### detachMethodRelationship

▸ **detachMethodRelationship**(`didUrl`, `relationship`): `boolean`

Detaches the given relationship from the given method, if the method exists.

#### Parameters

| Name | Type |
| :------ | :------ |
| `didUrl` | [`DIDUrl`](identity_wasm.DIDUrl.md) |
| `relationship` | [`MethodRelationship`](../enums/identity_wasm.MethodRelationship.md) |

#### Returns

`boolean`

___

### verifyJws

▸ **verifyJws**(`jws`, `options`, `signatureVerifier?`, `detachedPayload?`): [`DecodedJws`](identity_wasm.DecodedJws.md)

Decodes and verifies the provided JWS according to the passed `options` and `signatureVerifier`.
If a `signatureVerifier` is provided it will be used when
verifying decoded JWS signatures, otherwise a default verifier capable of handling the `EdDSA`, `ES256`, `ES256K`
algorithms will be used.

Regardless of which options are passed the following conditions must be met in order for a verification attempt to
take place.
- The JWS must be encoded according to the JWS compact serialization.
- The `kid` value in the protected header must be an identifier of a verification method in this DID document,
or set explicitly in the `options`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `jws` | [`Jws`](identity_wasm.Jws.md) |
| `options` | [`JwsVerificationOptions`](identity_wasm.JwsVerificationOptions.md) |
| `signatureVerifier?` | [`IJwsVerifier`](../interfaces/identity_wasm.IJwsVerifier.md) |
| `detachedPayload?` | `string` |

#### Returns

[`DecodedJws`](identity_wasm.DecodedJws.md)

___

### revokeCredentials

▸ **revokeCredentials**(`serviceQuery`, `indices`): `void`

If the document has a [RevocationBitmap](identity_wasm.RevocationBitmap.md) service identified by `serviceQuery`,
revoke all specified `indices`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `serviceQuery` | `string` \| [`DIDUrl`](identity_wasm.DIDUrl.md) |
| `indices` | `number` \| `number`[] |

#### Returns

`void`

___

### unrevokeCredentials

▸ **unrevokeCredentials**(`serviceQuery`, `indices`): `void`

If the document has a [RevocationBitmap](identity_wasm.RevocationBitmap.md) service identified by `serviceQuery`,
unrevoke all specified `indices`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `serviceQuery` | `string` \| [`DIDUrl`](identity_wasm.DIDUrl.md) |
| `indices` | `number` \| `number`[] |

#### Returns

`void`

___

### clone

▸ **clone**(): [`CoreDocument`](identity_wasm.CoreDocument.md)

Deep clones the [CoreDocument](identity_wasm.CoreDocument.md).

#### Returns

[`CoreDocument`](identity_wasm.CoreDocument.md)

___

### \_shallowCloneInternal

▸ **_shallowCloneInternal**(): [`CoreDocument`](identity_wasm.CoreDocument.md)

### Warning
This is for internal use only. Do not rely on or call this method.

#### Returns

[`CoreDocument`](identity_wasm.CoreDocument.md)

___

### \_strongCountInternal

▸ **_strongCountInternal**(): `number`

### Warning
This is for internal use only. Do not rely on or call this method.

#### Returns

`number`

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`CoreDocument`](identity_wasm.CoreDocument.md)

Deserializes an instance from a plain JS representation.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`CoreDocument`](identity_wasm.CoreDocument.md)

___

### generateMethod

▸ **generateMethod**(`storage`, `keyType`, `alg`, `fragment`, `scope`): `Promise`\<`string`\>

Generate new key material in the given `storage` and insert a new verification method with the corresponding
public key material into the DID document.

- If no fragment is given the `kid` of the generated JWK is used, if it is set, otherwise an error is returned.
- The `keyType` must be compatible with the given `storage`. `Storage`s are expected to export key type constants
for that use case.

The fragment of the generated method is returned.

#### Parameters

| Name | Type |
| :------ | :------ |
| `storage` | [`Storage`](identity_wasm.Storage.md) |
| `keyType` | `string` |
| `alg` | [`JwsAlgorithm`](../enums/jose_jws_algorithm.JwsAlgorithm.md) |
| `fragment` | `undefined` \| `string` |
| `scope` | [`MethodScope`](identity_wasm.MethodScope.md) |

#### Returns

`Promise`\<`string`\>

___

### purgeMethod

▸ **purgeMethod**(`storage`, `id`): `Promise`\<`void`\>

Remove the method identified by the `fragment` from the document and delete the corresponding key material in
the `storage`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `storage` | [`Storage`](identity_wasm.Storage.md) |
| `id` | [`DIDUrl`](identity_wasm.DIDUrl.md) |

#### Returns

`Promise`\<`void`\>

___

### createJws

▸ **createJws**(`storage`, `fragment`, `payload`, `options`): `Promise`\<[`Jws`](identity_wasm.Jws.md)\>

Sign the `payload` according to `options` with the storage backed private key corresponding to the public key
material in the verification method identified by the given `fragment.

Upon success a string representing a JWS encoded according to the Compact JWS Serialization format is returned.
See [RFC7515 section 3.1](https://www.rfc-editor.org/rfc/rfc7515#section-3.1).

#### Parameters

| Name | Type |
| :------ | :------ |
| `storage` | [`Storage`](identity_wasm.Storage.md) |
| `fragment` | `string` |
| `payload` | `string` |
| `options` | [`JwsSignatureOptions`](identity_wasm.JwsSignatureOptions.md) |

#### Returns

`Promise`\<[`Jws`](identity_wasm.Jws.md)\>

___

### createCredentialJwt

▸ **createCredentialJwt**(`storage`, `fragment`, `credential`, `options`, `custom_claims?`): `Promise`\<[`Jwt`](identity_wasm.Jwt.md)\>

Produces a JWT where the payload is produced from the given `credential`
in accordance with [VC Data Model v1.1](https://www.w3.org/TR/vc-data-model/#json-web-token).

Unless the `kid` is explicitly set in the options, the `kid` in the protected header is the `id`
of the method identified by `fragment` and the JWS signature will be produced by the corresponding
private key backed by the `storage` in accordance with the passed `options`.

The `custom_claims` can be used to set additional claims on the resulting JWT.

#### Parameters

| Name | Type |
| :------ | :------ |
| `storage` | [`Storage`](identity_wasm.Storage.md) |
| `fragment` | `string` |
| `credential` | [`Credential`](identity_wasm.Credential.md) |
| `options` | [`JwsSignatureOptions`](identity_wasm.JwsSignatureOptions.md) |
| `custom_claims?` | `Record`\<`string`, `any`\> |

#### Returns

`Promise`\<[`Jwt`](identity_wasm.Jwt.md)\>

___

### createPresentationJwt

▸ **createPresentationJwt**(`storage`, `fragment`, `presentation`, `signature_options`, `presentation_options`): `Promise`\<[`Jwt`](identity_wasm.Jwt.md)\>

Produces a JWT where the payload is produced from the given presentation.
in accordance with [VC Data Model v1.1](https://www.w3.org/TR/vc-data-model/#json-web-token).

Unless the `kid` is explicitly set in the options, the `kid` in the protected header is the `id`
of the method identified by `fragment` and the JWS signature will be produced by the corresponding
private key backed by the `storage` in accordance with the passed `options`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `storage` | [`Storage`](identity_wasm.Storage.md) |
| `fragment` | `string` |
| `presentation` | [`Presentation`](identity_wasm.Presentation.md) |
| `signature_options` | [`JwsSignatureOptions`](identity_wasm.JwsSignatureOptions.md) |
| `presentation_options` | [`JwtPresentationOptions`](identity_wasm.JwtPresentationOptions.md) |

#### Returns

`Promise`\<[`Jwt`](identity_wasm.Jwt.md)\>

___

### expandDIDJwk

▸ `Static` **expandDIDJwk**(`did`): [`CoreDocument`](identity_wasm.CoreDocument.md)

Creates a [CoreDocument](identity_wasm.CoreDocument.md) from the given [DIDJwk](identity_wasm.DIDJwk.md).

#### Parameters

| Name | Type |
| :------ | :------ |
| `did` | [`DIDJwk`](identity_wasm.DIDJwk.md) |

#### Returns

[`CoreDocument`](identity_wasm.CoreDocument.md)
