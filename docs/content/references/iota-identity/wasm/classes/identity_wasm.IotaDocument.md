# Class: IotaDocument

[identity\_wasm](../modules/identity_wasm.md).IotaDocument

A DID Document adhering to the IOTA DID method specification.

Note: All methods that involve reading from this class may potentially raise an error
if the object is being concurrently modified.

## Table of contents

### Constructors

- [constructor](identity_wasm.IotaDocument.md#constructor)

### Methods

- [toJSON](identity_wasm.IotaDocument.md#tojson)
- [toString](identity_wasm.IotaDocument.md#tostring)
- [newWithId](identity_wasm.IotaDocument.md#newwithid)
- [id](identity_wasm.IotaDocument.md#id)
- [controller](identity_wasm.IotaDocument.md#controller)
- [setController](identity_wasm.IotaDocument.md#setcontroller)
- [alsoKnownAs](identity_wasm.IotaDocument.md#alsoknownas)
- [setAlsoKnownAs](identity_wasm.IotaDocument.md#setalsoknownas)
- [properties](identity_wasm.IotaDocument.md#properties)
- [setPropertyUnchecked](identity_wasm.IotaDocument.md#setpropertyunchecked)
- [service](identity_wasm.IotaDocument.md#service)
- [insertService](identity_wasm.IotaDocument.md#insertservice)
- [removeService](identity_wasm.IotaDocument.md#removeservice)
- [resolveService](identity_wasm.IotaDocument.md#resolveservice)
- [methods](identity_wasm.IotaDocument.md#methods)
- [insertMethod](identity_wasm.IotaDocument.md#insertmethod)
- [removeMethod](identity_wasm.IotaDocument.md#removemethod)
- [resolveMethod](identity_wasm.IotaDocument.md#resolvemethod)
- [attachMethodRelationship](identity_wasm.IotaDocument.md#attachmethodrelationship)
- [detachMethodRelationship](identity_wasm.IotaDocument.md#detachmethodrelationship)
- [verifyJws](identity_wasm.IotaDocument.md#verifyjws)
- [pack](identity_wasm.IotaDocument.md#pack)
- [packWithEncoding](identity_wasm.IotaDocument.md#packwithencoding)
- [unpackFromOutput](identity_wasm.IotaDocument.md#unpackfromoutput)
- [unpackFromBlock](identity_wasm.IotaDocument.md#unpackfromblock)
- [metadata](identity_wasm.IotaDocument.md#metadata)
- [metadataCreated](identity_wasm.IotaDocument.md#metadatacreated)
- [setMetadataCreated](identity_wasm.IotaDocument.md#setmetadatacreated)
- [metadataUpdated](identity_wasm.IotaDocument.md#metadataupdated)
- [setMetadataUpdated](identity_wasm.IotaDocument.md#setmetadataupdated)
- [metadataDeactivated](identity_wasm.IotaDocument.md#metadatadeactivated)
- [setMetadataDeactivated](identity_wasm.IotaDocument.md#setmetadatadeactivated)
- [metadataStateControllerAddress](identity_wasm.IotaDocument.md#metadatastatecontrolleraddress)
- [metadataGovernorAddress](identity_wasm.IotaDocument.md#metadatagovernoraddress)
- [setMetadataPropertyUnchecked](identity_wasm.IotaDocument.md#setmetadatapropertyunchecked)
- [revokeCredentials](identity_wasm.IotaDocument.md#revokecredentials)
- [unrevokeCredentials](identity_wasm.IotaDocument.md#unrevokecredentials)
- [clone](identity_wasm.IotaDocument.md#clone)
- [\_shallowCloneInternal](identity_wasm.IotaDocument.md#_shallowcloneinternal)
- [\_strongCountInternal](identity_wasm.IotaDocument.md#_strongcountinternal)
- [fromJSON](identity_wasm.IotaDocument.md#fromjson)
- [toCoreDocument](identity_wasm.IotaDocument.md#tocoredocument)
- [generateMethod](identity_wasm.IotaDocument.md#generatemethod)
- [purgeMethod](identity_wasm.IotaDocument.md#purgemethod)
- [createJwt](identity_wasm.IotaDocument.md#createjwt)
- [createJws](identity_wasm.IotaDocument.md#createjws)
- [createCredentialJwt](identity_wasm.IotaDocument.md#createcredentialjwt)
- [createPresentationJwt](identity_wasm.IotaDocument.md#createpresentationjwt)
- [generateMethodJwp](identity_wasm.IotaDocument.md#generatemethodjwp)
- [createIssuedJwp](identity_wasm.IotaDocument.md#createissuedjwp)
- [createPresentedJwp](identity_wasm.IotaDocument.md#createpresentedjwp)
- [createCredentialJpt](identity_wasm.IotaDocument.md#createcredentialjpt)
- [createPresentationJpt](identity_wasm.IotaDocument.md#createpresentationjpt)

## Constructors

### constructor

• **new IotaDocument**(`network`)

Constructs an empty IOTA DID Document with a [placeholder](identity_wasm.IotaDID.md#placeholder) identifier
for the given `network`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `network` | `string` |

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

### newWithId

▸ `Static` **newWithId**(`id`): [`IotaDocument`](identity_wasm.IotaDocument.md)

Constructs an empty DID Document with the given identifier.

#### Parameters

| Name | Type |
| :------ | :------ |
| `id` | [`IotaDID`](identity_wasm.IotaDID.md) |

#### Returns

[`IotaDocument`](identity_wasm.IotaDocument.md)

___

### id

▸ **id**(): [`IotaDID`](identity_wasm.IotaDID.md)

Returns a copy of the DID Document `id`.

#### Returns

[`IotaDID`](identity_wasm.IotaDID.md)

___

### controller

▸ **controller**(): [`IotaDID`](identity_wasm.IotaDID.md)[]

Returns a copy of the list of document controllers.

NOTE: controllers are determined by the `state_controller` unlock condition of the output
during resolution and are omitted when publishing.

#### Returns

[`IotaDID`](identity_wasm.IotaDID.md)[]

___

### setController

▸ **setController**(`controller`): `void`

Sets the controllers of the document.

Note: Duplicates will be ignored.
Use `null` to remove all controllers.

#### Parameters

| Name | Type |
| :------ | :------ |
| `controller` | ``null`` \| [`IotaDID`](identity_wasm.IotaDID.md)[] |

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

Return a set of all [Service](identity_wasm.Service.md) in the document.

#### Returns

[`Service`](identity_wasm.Service.md)[]

___

### insertService

▸ **insertService**(`service`): `void`

Add a new [Service](identity_wasm.Service.md) to the document.

Returns `true` if the service was added.

#### Parameters

| Name | Type |
| :------ | :------ |
| `service` | [`Service`](identity_wasm.Service.md) |

#### Returns

`void`

___

### removeService

▸ **removeService**(`did`): `undefined` \| [`Service`](identity_wasm.Service.md)

Remove a [Service](identity_wasm.Service.md) identified by the given [DIDUrl](identity_wasm.DIDUrl.md) from the document.

Returns `true` if a service was removed.

#### Parameters

| Name | Type |
| :------ | :------ |
| `did` | [`DIDUrl`](identity_wasm.DIDUrl.md) |

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
- The `kid` value in the protected header must be an identifier of a verification method in this DID document.

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

### pack

▸ **pack**(): `Uint8Array`

Serializes the document for inclusion in an Alias Output's state metadata
with the default [StateMetadataEncoding](../enums/identity_wasm.StateMetadataEncoding.md).

#### Returns

`Uint8Array`

___

### packWithEncoding

▸ **packWithEncoding**(`encoding`): `Uint8Array`

Serializes the document for inclusion in an Alias Output's state metadata.

#### Parameters

| Name | Type |
| :------ | :------ |
| `encoding` | [`Json`](../enums/identity_wasm.StateMetadataEncoding.md#json) |

#### Returns

`Uint8Array`

___

### unpackFromOutput

▸ `Static` **unpackFromOutput**(`did`, `aliasOutput`, `allowEmpty`): [`IotaDocument`](identity_wasm.IotaDocument.md)

Deserializes the document from an Alias Output.

If `allowEmpty` is true, this will return an empty DID document marked as `deactivated`
if `stateMetadata` is empty.

The `tokenSupply` must be equal to the token supply of the network the DID is associated with.  

NOTE: `did` is required since it is omitted from the serialized DID Document and
cannot be inferred from the state metadata. It also indicates the network, which is not
encoded in the `AliasId` alone.

#### Parameters

| Name | Type |
| :------ | :------ |
| `did` | [`IotaDID`](identity_wasm.IotaDID.md) |
| `aliasOutput` | `AliasOutputBuilderParams` |
| `allowEmpty` | `boolean` |

#### Returns

[`IotaDocument`](identity_wasm.IotaDocument.md)

___

### unpackFromBlock

▸ `Static` **unpackFromBlock**(`network`, `block`): [`IotaDocument`](identity_wasm.IotaDocument.md)[]

Returns all DID documents of the Alias Outputs contained in the block's transaction payload
outputs, if any.

Errors if any Alias Output does not contain a valid or empty DID Document.

#### Parameters

| Name | Type |
| :------ | :------ |
| `network` | `string` |
| `block` | `Block` |

#### Returns

[`IotaDocument`](identity_wasm.IotaDocument.md)[]

___

### metadata

▸ **metadata**(): [`IotaDocumentMetadata`](identity_wasm.IotaDocumentMetadata.md)

Returns a copy of the metadata associated with this document.

NOTE: Copies all the metadata. See also `metadataCreated`, `metadataUpdated`,
`metadataPreviousMessageId`, `metadataProof` if only a subset of the metadata required.

#### Returns

[`IotaDocumentMetadata`](identity_wasm.IotaDocumentMetadata.md)

___

### metadataCreated

▸ **metadataCreated**(): `undefined` \| [`Timestamp`](identity_wasm.Timestamp.md)

Returns a copy of the timestamp of when the DID document was created.

#### Returns

`undefined` \| [`Timestamp`](identity_wasm.Timestamp.md)

___

### setMetadataCreated

▸ **setMetadataCreated**(`timestamp`): `void`

Sets the timestamp of when the DID document was created.

#### Parameters

| Name | Type |
| :------ | :------ |
| `timestamp` | `undefined` \| [`Timestamp`](identity_wasm.Timestamp.md) |

#### Returns

`void`

___

### metadataUpdated

▸ **metadataUpdated**(): `undefined` \| [`Timestamp`](identity_wasm.Timestamp.md)

Returns a copy of the timestamp of the last DID document update.

#### Returns

`undefined` \| [`Timestamp`](identity_wasm.Timestamp.md)

___

### setMetadataUpdated

▸ **setMetadataUpdated**(`timestamp`): `void`

Sets the timestamp of the last DID document update.

#### Parameters

| Name | Type |
| :------ | :------ |
| `timestamp` | `undefined` \| [`Timestamp`](identity_wasm.Timestamp.md) |

#### Returns

`void`

___

### metadataDeactivated

▸ **metadataDeactivated**(): `undefined` \| `boolean`

Returns a copy of the deactivated status of the DID document.

#### Returns

`undefined` \| `boolean`

___

### setMetadataDeactivated

▸ **setMetadataDeactivated**(`deactivated?`): `void`

Sets the deactivated status of the DID document.

#### Parameters

| Name | Type |
| :------ | :------ |
| `deactivated?` | `boolean` |

#### Returns

`void`

___

### metadataStateControllerAddress

▸ **metadataStateControllerAddress**(): `undefined` \| `string`

Returns a copy of the Bech32-encoded state controller address, if present.

#### Returns

`undefined` \| `string`

___

### metadataGovernorAddress

▸ **metadataGovernorAddress**(): `undefined` \| `string`

Returns a copy of the Bech32-encoded governor address, if present.

#### Returns

`undefined` \| `string`

___

### setMetadataPropertyUnchecked

▸ **setMetadataPropertyUnchecked**(`key`, `value`): `void`

Sets a custom property in the document metadata.
If the value is set to `null`, the custom property will be removed.

#### Parameters

| Name | Type |
| :------ | :------ |
| `key` | `string` |
| `value` | `any` |

#### Returns

`void`

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

▸ **clone**(): [`IotaDocument`](identity_wasm.IotaDocument.md)

Returns a deep clone of the [IotaDocument](identity_wasm.IotaDocument.md).

#### Returns

[`IotaDocument`](identity_wasm.IotaDocument.md)

___

### \_shallowCloneInternal

▸ **_shallowCloneInternal**(): [`IotaDocument`](identity_wasm.IotaDocument.md)

### Warning
This is for internal use only. Do not rely on or call this method.

#### Returns

[`IotaDocument`](identity_wasm.IotaDocument.md)

___

### \_strongCountInternal

▸ **_strongCountInternal**(): `number`

### Warning
This is for internal use only. Do not rely on or call this method.

#### Returns

`number`

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`IotaDocument`](identity_wasm.IotaDocument.md)

Deserializes an instance from a plain JS representation.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`IotaDocument`](identity_wasm.IotaDocument.md)

___

### toCoreDocument

▸ **toCoreDocument**(): [`CoreDocument`](identity_wasm.CoreDocument.md)

Transforms the [IotaDocument](identity_wasm.IotaDocument.md) to its [CoreDocument](identity_wasm.CoreDocument.md) representation.

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

Remove the method identified by the given fragment from the document and delete the corresponding key material in
the given `storage`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `storage` | [`Storage`](identity_wasm.Storage.md) |
| `id` | [`DIDUrl`](identity_wasm.DIDUrl.md) |

#### Returns

`Promise`\<`void`\>

___

### createJwt

▸ **createJwt**(`storage`, `fragment`, `payload`, `options`): `Promise`\<[`Jws`](identity_wasm.Jws.md)\>

Sign the `payload` according to `options` with the storage backed private key corresponding to the public key
material in the verification method identified by the given `fragment.

Upon success a string representing a JWS encoded according to the Compact JWS Serialization format is returned.
See [RFC7515 section 3.1](https://www.rfc-editor.org/rfc/rfc7515#section-3.1).

@deprecated Use `createJws()` instead.

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

Produces a JWS where the payload is produced from the given `credential`
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

### generateMethodJwp

▸ **generateMethodJwp**(`storage`, `alg`, `fragment`, `scope`): `Promise`\<`string`\>

#### Parameters

| Name | Type |
| :------ | :------ |
| `storage` | [`Storage`](identity_wasm.Storage.md) |
| `alg` | [`ProofAlgorithm`](../enums/identity_wasm.ProofAlgorithm.md) |
| `fragment` | `undefined` \| `string` |
| `scope` | [`MethodScope`](identity_wasm.MethodScope.md) |

#### Returns

`Promise`\<`string`\>

___

### createIssuedJwp

▸ **createIssuedJwp**(`storage`, `fragment`, `jpt_claims`, `options`): `Promise`\<`string`\>

#### Parameters

| Name | Type |
| :------ | :------ |
| `storage` | [`Storage`](identity_wasm.Storage.md) |
| `fragment` | `string` |
| `jpt_claims` | [`JptClaims`](../interfaces/identity_wasm.JptClaims.md) |
| `options` | [`JwpCredentialOptions`](identity_wasm.JwpCredentialOptions.md) |

#### Returns

`Promise`\<`string`\>

___

### createPresentedJwp

▸ **createPresentedJwp**(`presentation`, `method_id`, `options`): `Promise`\<`string`\>

#### Parameters

| Name | Type |
| :------ | :------ |
| `presentation` | [`SelectiveDisclosurePresentation`](identity_wasm.SelectiveDisclosurePresentation.md) |
| `method_id` | `string` |
| `options` | [`JwpPresentationOptions`](identity_wasm.JwpPresentationOptions.md) |

#### Returns

`Promise`\<`string`\>

___

### createCredentialJpt

▸ **createCredentialJpt**(`credential`, `storage`, `fragment`, `options`, `custom_claims?`): `Promise`\<[`Jpt`](identity_wasm.Jpt.md)\>

#### Parameters

| Name | Type |
| :------ | :------ |
| `credential` | [`Credential`](identity_wasm.Credential.md) |
| `storage` | [`Storage`](identity_wasm.Storage.md) |
| `fragment` | `string` |
| `options` | [`JwpCredentialOptions`](identity_wasm.JwpCredentialOptions.md) |
| `custom_claims?` | `Map`\<`string`, `any`\> |

#### Returns

`Promise`\<[`Jpt`](identity_wasm.Jpt.md)\>

___

### createPresentationJpt

▸ **createPresentationJpt**(`presentation`, `method_id`, `options`): `Promise`\<[`Jpt`](identity_wasm.Jpt.md)\>

#### Parameters

| Name | Type |
| :------ | :------ |
| `presentation` | [`SelectiveDisclosurePresentation`](identity_wasm.SelectiveDisclosurePresentation.md) |
| `method_id` | `string` |
| `options` | [`JwpPresentationOptions`](identity_wasm.JwpPresentationOptions.md) |

#### Returns

`Promise`\<[`Jpt`](identity_wasm.Jpt.md)\>
