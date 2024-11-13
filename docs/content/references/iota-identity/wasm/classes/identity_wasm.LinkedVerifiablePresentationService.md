# Class: LinkedVerifiablePresentationService

[identity\_wasm](../modules/identity_wasm.md).LinkedVerifiablePresentationService

## Table of contents

### Constructors

- [constructor](identity_wasm.LinkedVerifiablePresentationService.md#constructor)

### Methods

- [toJSON](identity_wasm.LinkedVerifiablePresentationService.md#tojson)
- [toString](identity_wasm.LinkedVerifiablePresentationService.md#tostring)
- [verifiablePresentationUrls](identity_wasm.LinkedVerifiablePresentationService.md#verifiablepresentationurls)
- [toService](identity_wasm.LinkedVerifiablePresentationService.md#toservice)
- [fromService](identity_wasm.LinkedVerifiablePresentationService.md#fromservice)
- [isValid](identity_wasm.LinkedVerifiablePresentationService.md#isvalid)
- [clone](identity_wasm.LinkedVerifiablePresentationService.md#clone)
- [fromJSON](identity_wasm.LinkedVerifiablePresentationService.md#fromjson)

## Constructors

### constructor

• **new LinkedVerifiablePresentationService**(`options`)

Constructs a new [LinkedVerifiablePresentationService](identity_wasm.LinkedVerifiablePresentationService.md) that wraps a spec compliant [Linked Verifiable Presentation Service Endpoint](https://identity.foundation/linked-vp/#linked-verifiable-presentation-service-endpoint).

#### Parameters

| Name | Type |
| :------ | :------ |
| `options` | [`ILinkedVerifiablePresentationService`](../interfaces/identity_wasm.ILinkedVerifiablePresentationService.md) |

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

### verifiablePresentationUrls

▸ **verifiablePresentationUrls**(): `string`[]

Returns the domains contained in the Linked Verifiable Presentation Service.

#### Returns

`string`[]

___

### toService

▸ **toService**(): [`Service`](identity_wasm.Service.md)

Returns the inner service which can be added to a DID Document.

#### Returns

[`Service`](identity_wasm.Service.md)

___

### fromService

▸ `Static` **fromService**(`service`): [`LinkedVerifiablePresentationService`](identity_wasm.LinkedVerifiablePresentationService.md)

Creates a new [LinkedVerifiablePresentationService](identity_wasm.LinkedVerifiablePresentationService.md) from a [Service](identity_wasm.Service.md).

# Error

Errors if `service` is not a valid Linked Verifiable Presentation Service.

#### Parameters

| Name | Type |
| :------ | :------ |
| `service` | [`Service`](identity_wasm.Service.md) |

#### Returns

[`LinkedVerifiablePresentationService`](identity_wasm.LinkedVerifiablePresentationService.md)

___

### isValid

▸ `Static` **isValid**(`service`): `boolean`

Returns `true` if a [Service](identity_wasm.Service.md) is a valid Linked Verifiable Presentation Service.

#### Parameters

| Name | Type |
| :------ | :------ |
| `service` | [`Service`](identity_wasm.Service.md) |

#### Returns

`boolean`

___

### clone

▸ **clone**(): [`LinkedVerifiablePresentationService`](identity_wasm.LinkedVerifiablePresentationService.md)

Deep clones the object.

#### Returns

[`LinkedVerifiablePresentationService`](identity_wasm.LinkedVerifiablePresentationService.md)

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`LinkedVerifiablePresentationService`](identity_wasm.LinkedVerifiablePresentationService.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`LinkedVerifiablePresentationService`](identity_wasm.LinkedVerifiablePresentationService.md)
