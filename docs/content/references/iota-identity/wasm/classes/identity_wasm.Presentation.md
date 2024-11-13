# Class: Presentation

[identity\_wasm](../modules/identity_wasm.md).Presentation

## Table of contents

### Constructors

- [constructor](identity_wasm.Presentation.md#constructor)

### Methods

- [toJSON](identity_wasm.Presentation.md#tojson)
- [toString](identity_wasm.Presentation.md#tostring)
- [BaseContext](identity_wasm.Presentation.md#basecontext)
- [BaseType](identity_wasm.Presentation.md#basetype)
- [context](identity_wasm.Presentation.md#context)
- [id](identity_wasm.Presentation.md#id)
- [type](identity_wasm.Presentation.md#type)
- [verifiableCredential](identity_wasm.Presentation.md#verifiablecredential)
- [holder](identity_wasm.Presentation.md#holder)
- [refreshService](identity_wasm.Presentation.md#refreshservice)
- [termsOfUse](identity_wasm.Presentation.md#termsofuse)
- [proof](identity_wasm.Presentation.md#proof)
- [setProof](identity_wasm.Presentation.md#setproof)
- [properties](identity_wasm.Presentation.md#properties)
- [fromJSON](identity_wasm.Presentation.md#fromjson)
- [clone](identity_wasm.Presentation.md#clone)

## Constructors

### constructor

• **new Presentation**(`values`)

Constructs a new presentation.

#### Parameters

| Name | Type |
| :------ | :------ |
| `values` | [`IPresentation`](../interfaces/identity_wasm.IPresentation.md) |

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

### context

▸ **context**(): (`string` \| `Record`\<`string`, `any`\>)[]

Returns a copy of the JSON-LD context(s) applicable to the presentation.

#### Returns

(`string` \| `Record`\<`string`, `any`\>)[]

___

### id

▸ **id**(): `undefined` \| `string`

Returns a copy of the unique `URI` identifying the presentation.

#### Returns

`undefined` \| `string`

___

### type

▸ **type**(): `string`[]

Returns a copy of the URIs defining the type of the presentation.

#### Returns

`string`[]

___

### verifiableCredential

▸ **verifiableCredential**(): [`UnknownCredential`](identity_wasm.UnknownCredential.md)[]

Returns the JWT credentials expressing the claims of the presentation.

#### Returns

[`UnknownCredential`](identity_wasm.UnknownCredential.md)[]

___

### holder

▸ **holder**(): `string`

Returns a copy of the URI of the entity that generated the presentation.

#### Returns

`string`

___

### refreshService

▸ **refreshService**(): [`RefreshService`](../interfaces/identity_wasm.RefreshService.md)[]

Returns a copy of the service(s) used to refresh an expired [Credential](identity_wasm.Credential.md) in the presentation.

#### Returns

[`RefreshService`](../interfaces/identity_wasm.RefreshService.md)[]

___

### termsOfUse

▸ **termsOfUse**(): [`Policy`](../interfaces/identity_wasm.Policy.md)[]

Returns a copy of the terms-of-use specified by the presentation holder

#### Returns

[`Policy`](../interfaces/identity_wasm.Policy.md)[]

___

### proof

▸ **proof**(): `undefined` \| [`Proof`](identity_wasm.Proof.md)

Optional cryptographic proof, unrelated to JWT.

#### Returns

`undefined` \| [`Proof`](identity_wasm.Proof.md)

___

### setProof

▸ **setProof**(`proof?`): `void`

Sets the proof property of the [Presentation](identity_wasm.Presentation.md).

Note that this proof is not related to JWT.

#### Parameters

| Name | Type |
| :------ | :------ |
| `proof?` | [`Proof`](identity_wasm.Proof.md) |

#### Returns

`void`

___

### properties

▸ **properties**(): `Map`\<`string`, `any`\>

Returns a copy of the miscellaneous properties on the presentation.

#### Returns

`Map`\<`string`, `any`\>

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`Presentation`](identity_wasm.Presentation.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`Presentation`](identity_wasm.Presentation.md)

___

### clone

▸ **clone**(): [`Presentation`](identity_wasm.Presentation.md)

Deep clones the object.

#### Returns

[`Presentation`](identity_wasm.Presentation.md)
