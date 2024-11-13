# Class: Credential

[identity\_wasm](../modules/identity_wasm.md).Credential

## Table of contents

### Constructors

- [constructor](identity_wasm.Credential.md#constructor)

### Methods

- [toJSON](identity_wasm.Credential.md#tojson)
- [toString](identity_wasm.Credential.md#tostring)
- [BaseContext](identity_wasm.Credential.md#basecontext)
- [BaseType](identity_wasm.Credential.md#basetype)
- [createDomainLinkageCredential](identity_wasm.Credential.md#createdomainlinkagecredential)
- [context](identity_wasm.Credential.md#context)
- [id](identity_wasm.Credential.md#id)
- [type](identity_wasm.Credential.md#type)
- [credentialSubject](identity_wasm.Credential.md#credentialsubject)
- [issuer](identity_wasm.Credential.md#issuer)
- [issuanceDate](identity_wasm.Credential.md#issuancedate)
- [expirationDate](identity_wasm.Credential.md#expirationdate)
- [credentialStatus](identity_wasm.Credential.md#credentialstatus)
- [credentialSchema](identity_wasm.Credential.md#credentialschema)
- [refreshService](identity_wasm.Credential.md#refreshservice)
- [termsOfUse](identity_wasm.Credential.md#termsofuse)
- [evidence](identity_wasm.Credential.md#evidence)
- [nonTransferable](identity_wasm.Credential.md#nontransferable)
- [proof](identity_wasm.Credential.md#proof)
- [properties](identity_wasm.Credential.md#properties)
- [setProof](identity_wasm.Credential.md#setproof)
- [toJwtClaims](identity_wasm.Credential.md#tojwtclaims)
- [fromJSON](identity_wasm.Credential.md#fromjson)
- [clone](identity_wasm.Credential.md#clone)

## Constructors

### constructor

• **new Credential**(`values`)

Constructs a new [Credential](identity_wasm.Credential.md).

#### Parameters

| Name | Type |
| :------ | :------ |
| `values` | [`ICredential`](../interfaces/identity_wasm.ICredential.md) |

## Methods

### toJSON

▸ **toJSON**(): `Object`

* Return copy of self without private attributes.

#### Returns

`Object`

▸ **toJSON**(): `any`

Serializes this to a JSON object.

#### Returns

`any`

___

### toString

▸ **toString**(): `string`

Return stringified version of self.

#### Returns

`string`

___

### BaseContext

▸ `Static` **BaseContext**(): `string`

Returns the base JSON-LD context.

#### Returns

`string`

___

### BaseType

▸ `Static` **BaseType**(): `string`

Returns the base type.

#### Returns

`string`

___

### createDomainLinkageCredential

▸ `Static` **createDomainLinkageCredential**(`values`): [`Credential`](identity_wasm.Credential.md)

#### Parameters

| Name | Type |
| :------ | :------ |
| `values` | [`IDomainLinkageCredential`](../interfaces/identity_wasm.IDomainLinkageCredential.md) |

#### Returns

[`Credential`](identity_wasm.Credential.md)

___

### context

▸ **context**(): (`string` \| `Record`\<`string`, `any`\>)[]

Returns a copy of the JSON-LD context(s) applicable to the [Credential](identity_wasm.Credential.md).

#### Returns

(`string` \| `Record`\<`string`, `any`\>)[]

___

### id

▸ **id**(): `undefined` \| `string`

Returns a copy of the unique `URI` identifying the [Credential](identity_wasm.Credential.md) .

#### Returns

`undefined` \| `string`

___

### type

▸ **type**(): `string`[]

Returns a copy of the URIs defining the type of the [Credential](identity_wasm.Credential.md).

#### Returns

`string`[]

___

### credentialSubject

▸ **credentialSubject**(): [`Subject`](../interfaces/identity_wasm.Subject.md)[]

Returns a copy of the [Credential](identity_wasm.Credential.md) subject(s).

#### Returns

[`Subject`](../interfaces/identity_wasm.Subject.md)[]

___

### issuer

▸ **issuer**(): `string` \| [`Issuer`](../interfaces/identity_wasm.Issuer.md)

Returns a copy of the issuer of the [Credential](identity_wasm.Credential.md).

#### Returns

`string` \| [`Issuer`](../interfaces/identity_wasm.Issuer.md)

___

### issuanceDate

▸ **issuanceDate**(): [`Timestamp`](identity_wasm.Timestamp.md)

Returns a copy of the timestamp of when the [Credential](identity_wasm.Credential.md) becomes valid.

#### Returns

[`Timestamp`](identity_wasm.Timestamp.md)

___

### expirationDate

▸ **expirationDate**(): `undefined` \| [`Timestamp`](identity_wasm.Timestamp.md)

Returns a copy of the timestamp of when the [Credential](identity_wasm.Credential.md) should no longer be considered valid.

#### Returns

`undefined` \| [`Timestamp`](identity_wasm.Timestamp.md)

___

### credentialStatus

▸ **credentialStatus**(): [`Status`](../interfaces/identity_wasm.Status.md)[]

Returns a copy of the information used to determine the current status of the [Credential](identity_wasm.Credential.md).

#### Returns

[`Status`](../interfaces/identity_wasm.Status.md)[]

___

### credentialSchema

▸ **credentialSchema**(): [`Schema`](../interfaces/identity_wasm.Schema.md)[]

Returns a copy of the information used to assist in the enforcement of a specific [Credential](identity_wasm.Credential.md) structure.

#### Returns

[`Schema`](../interfaces/identity_wasm.Schema.md)[]

___

### refreshService

▸ **refreshService**(): [`RefreshService`](../interfaces/identity_wasm.RefreshService.md)[]

Returns a copy of the service(s) used to refresh an expired [Credential](identity_wasm.Credential.md).

#### Returns

[`RefreshService`](../interfaces/identity_wasm.RefreshService.md)[]

___

### termsOfUse

▸ **termsOfUse**(): [`Policy`](../interfaces/identity_wasm.Policy.md)[]

Returns a copy of the terms-of-use specified by the [Credential](identity_wasm.Credential.md) issuer.

#### Returns

[`Policy`](../interfaces/identity_wasm.Policy.md)[]

___

### evidence

▸ **evidence**(): [`Evidence`](../interfaces/identity_wasm.Evidence.md)[]

Returns a copy of the human-readable evidence used to support the claims within the [Credential](identity_wasm.Credential.md).

#### Returns

[`Evidence`](../interfaces/identity_wasm.Evidence.md)[]

___

### nonTransferable

▸ **nonTransferable**(): `undefined` \| `boolean`

Returns whether or not the [Credential](identity_wasm.Credential.md) must only be contained within a  [Presentation](identity_wasm.Presentation.md)
with a proof issued from the [Credential](identity_wasm.Credential.md) subject.

#### Returns

`undefined` \| `boolean`

___

### proof

▸ **proof**(): `undefined` \| [`Proof`](identity_wasm.Proof.md)

Optional cryptographic proof, unrelated to JWT.

#### Returns

`undefined` \| [`Proof`](identity_wasm.Proof.md)

___

### properties

▸ **properties**(): `Map`\<`string`, `any`\>

Returns a copy of the miscellaneous properties on the [Credential](identity_wasm.Credential.md).

#### Returns

`Map`\<`string`, `any`\>

___

### setProof

▸ **setProof**(`proof?`): `void`

Sets the `proof` property of the [Credential](identity_wasm.Credential.md).

Note that this proof is not related to JWT.

#### Parameters

| Name | Type |
| :------ | :------ |
| `proof?` | [`Proof`](identity_wasm.Proof.md) |

#### Returns

`void`

___

### toJwtClaims

▸ **toJwtClaims**(`custom_claims?`): `Record`\<`string`, `any`\>

Serializes the `Credential` as a JWT claims set
in accordance with [VC Data Model v1.1](https://www.w3.org/TR/vc-data-model/#json-web-token).

The resulting object can be used as the payload of a JWS when issuing the credential.

#### Parameters

| Name | Type |
| :------ | :------ |
| `custom_claims?` | `Record`\<`string`, `any`\> |

#### Returns

`Record`\<`string`, `any`\>

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`Credential`](identity_wasm.Credential.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`Credential`](identity_wasm.Credential.md)

___

### clone

▸ **clone**(): [`Credential`](identity_wasm.Credential.md)

Deep clones the object.

#### Returns

[`Credential`](identity_wasm.Credential.md)
