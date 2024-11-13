# Class: StatusList2021Entry

[identity\_wasm](../modules/identity_wasm.md).StatusList2021Entry

[StatusList2021Entry](https://www.w3.org/TR/2023/WD-vc-status-list-20230427/#statuslist2021entry) implementation.

## Table of contents

### Constructors

- [constructor](identity_wasm.StatusList2021Entry.md#constructor)

### Methods

- [toJSON](identity_wasm.StatusList2021Entry.md#tojson)
- [toString](identity_wasm.StatusList2021Entry.md#tostring)
- [id](identity_wasm.StatusList2021Entry.md#id)
- [purpose](identity_wasm.StatusList2021Entry.md#purpose)
- [index](identity_wasm.StatusList2021Entry.md#index)
- [statusListCredential](identity_wasm.StatusList2021Entry.md#statuslistcredential)
- [toStatus](identity_wasm.StatusList2021Entry.md#tostatus)
- [clone](identity_wasm.StatusList2021Entry.md#clone)
- [fromJSON](identity_wasm.StatusList2021Entry.md#fromjson)

## Constructors

### constructor

• **new StatusList2021Entry**(`status_list`, `purpose`, `index`, `id?`)

Creates a new [StatusList2021Entry](identity_wasm.StatusList2021Entry.md).

#### Parameters

| Name | Type |
| :------ | :------ |
| `status_list` | `string` |
| `purpose` | [`StatusPurpose`](../enums/identity_wasm.StatusPurpose.md) |
| `index` | `number` |
| `id?` | `string` |

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

▸ **id**(): `string`

Returns this `credentialStatus`'s `id`.

#### Returns

`string`

___

### purpose

▸ **purpose**(): [`StatusPurpose`](../enums/identity_wasm.StatusPurpose.md)

Returns the purpose of this entry.

#### Returns

[`StatusPurpose`](../enums/identity_wasm.StatusPurpose.md)

___

### index

▸ **index**(): `number`

Returns the index of this entry.

#### Returns

`number`

___

### statusListCredential

▸ **statusListCredential**(): `string`

Returns the referenced [StatusList2021Credential](identity_wasm.StatusList2021Credential.md)'s url.

#### Returns

`string`

___

### toStatus

▸ **toStatus**(): [`Status`](../interfaces/identity_wasm.Status.md)

Downcasts this to [Status](../interfaces/identity_wasm.Status.md)

#### Returns

[`Status`](../interfaces/identity_wasm.Status.md)

___

### clone

▸ **clone**(): [`StatusList2021Entry`](identity_wasm.StatusList2021Entry.md)

Deep clones the object.

#### Returns

[`StatusList2021Entry`](identity_wasm.StatusList2021Entry.md)

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`StatusList2021Entry`](identity_wasm.StatusList2021Entry.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`StatusList2021Entry`](identity_wasm.StatusList2021Entry.md)
