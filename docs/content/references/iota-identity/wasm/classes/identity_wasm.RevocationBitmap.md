# Class: RevocationBitmap

[identity\_wasm](../modules/identity_wasm.md).RevocationBitmap

A compressed bitmap for managing credential revocation.

## Table of contents

### Constructors

- [constructor](identity_wasm.RevocationBitmap.md#constructor)

### Methods

- [toJSON](identity_wasm.RevocationBitmap.md#tojson)
- [toString](identity_wasm.RevocationBitmap.md#tostring)
- [type](identity_wasm.RevocationBitmap.md#type)
- [isRevoked](identity_wasm.RevocationBitmap.md#isrevoked)
- [revoke](identity_wasm.RevocationBitmap.md#revoke)
- [unrevoke](identity_wasm.RevocationBitmap.md#unrevoke)
- [len](identity_wasm.RevocationBitmap.md#len)
- [toService](identity_wasm.RevocationBitmap.md#toservice)
- [fromEndpoint](identity_wasm.RevocationBitmap.md#fromendpoint)

## Constructors

### constructor

• **new RevocationBitmap**()

Creates a new [RevocationBitmap](identity_wasm.RevocationBitmap.md) instance.

## Methods

### toJSON

▸ **toJSON**(): `Object`

* Return copy of self without private attributes.

#### Returns

`Object`

___

### toString

▸ **toString**(): `string`

Return stringified version of self.

#### Returns

`string`

___

### type

▸ `Static` **type**(): `string`

The name of the service type.

#### Returns

`string`

___

### isRevoked

▸ **isRevoked**(`index`): `boolean`

Returns `true` if the credential at the given `index` is revoked.

#### Parameters

| Name | Type |
| :------ | :------ |
| `index` | `number` |

#### Returns

`boolean`

___

### revoke

▸ **revoke**(`index`): `boolean`

Mark the given index as revoked.

Returns true if the index was absent from the set.

#### Parameters

| Name | Type |
| :------ | :------ |
| `index` | `number` |

#### Returns

`boolean`

___

### unrevoke

▸ **unrevoke**(`index`): `boolean`

Mark the index as not revoked.

Returns true if the index was present in the set.

#### Parameters

| Name | Type |
| :------ | :------ |
| `index` | `number` |

#### Returns

`boolean`

___

### len

▸ **len**(): `number`

Returns the number of revoked credentials.

#### Returns

`number`

___

### toService

▸ **toService**(`serviceId`): [`Service`](identity_wasm.Service.md)

Return a `Service` with:
- the service's id set to `serviceId`,
- of type `RevocationBitmap2022`,
- and with the bitmap embedded in a data url in the service's endpoint.

#### Parameters

| Name | Type |
| :------ | :------ |
| `serviceId` | [`DIDUrl`](identity_wasm.DIDUrl.md) |

#### Returns

[`Service`](identity_wasm.Service.md)

___

### fromEndpoint

▸ `Static` **fromEndpoint**(`service`): [`RevocationBitmap`](identity_wasm.RevocationBitmap.md)

Try to construct a [RevocationBitmap](identity_wasm.RevocationBitmap.md) from a service
if it is a valid Revocation Bitmap Service.

#### Parameters

| Name | Type |
| :------ | :------ |
| `service` | [`Service`](identity_wasm.Service.md) |

#### Returns

[`RevocationBitmap`](identity_wasm.RevocationBitmap.md)
