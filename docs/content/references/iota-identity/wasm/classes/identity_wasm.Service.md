# Class: Service

[identity\_wasm](../modules/identity_wasm.md).Service

A DID Document Service used to enable trusted interactions associated with a DID subject.

## Table of contents

### Constructors

- [constructor](identity_wasm.Service.md#constructor)

### Methods

- [toJSON](identity_wasm.Service.md#tojson)
- [toString](identity_wasm.Service.md#tostring)
- [id](identity_wasm.Service.md#id)
- [type](identity_wasm.Service.md#type)
- [serviceEndpoint](identity_wasm.Service.md#serviceendpoint)
- [properties](identity_wasm.Service.md#properties)
- [fromJSON](identity_wasm.Service.md#fromjson)
- [clone](identity_wasm.Service.md#clone)

## Constructors

### constructor

• **new Service**(`service`)

#### Parameters

| Name | Type |
| :------ | :------ |
| `service` | [`IService`](../interfaces/identity_wasm.IService.md) |

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

### id

▸ **id**(): [`DIDUrl`](identity_wasm.DIDUrl.md)

Returns a copy of the [Service](identity_wasm.Service.md) id.

#### Returns

[`DIDUrl`](identity_wasm.DIDUrl.md)

___

### type

▸ **type**(): `string`[]

Returns a copy of the [Service](identity_wasm.Service.md) type.

#### Returns

`string`[]

___

### serviceEndpoint

▸ **serviceEndpoint**(): `string` \| `string`[] \| `Map`\<`string`, `string`[]\>

Returns a copy of the [Service](identity_wasm.Service.md) endpoint.

#### Returns

`string` \| `string`[] \| `Map`\<`string`, `string`[]\>

___

### properties

▸ **properties**(): `Map`\<`string`, `any`\>

Returns a copy of the custom properties on the [Service](identity_wasm.Service.md).

#### Returns

`Map`\<`string`, `any`\>

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`Service`](identity_wasm.Service.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`Service`](identity_wasm.Service.md)

___

### clone

▸ **clone**(): [`Service`](identity_wasm.Service.md)

Deep clones the object.

#### Returns

[`Service`](identity_wasm.Service.md)
