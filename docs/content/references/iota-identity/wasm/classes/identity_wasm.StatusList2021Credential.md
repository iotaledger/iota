# Class: StatusList2021Credential

[identity\_wasm](../modules/identity_wasm.md).StatusList2021Credential

A parsed [StatusList2021Credential](https://www.w3.org/TR/2023/WD-vc-status-list-20230427/#statuslist2021credential).

## Table of contents

### Constructors

- [constructor](identity_wasm.StatusList2021Credential.md#constructor)

### Methods

- [toJSON](identity_wasm.StatusList2021Credential.md#tojson)
- [toString](identity_wasm.StatusList2021Credential.md#tostring)
- [id](identity_wasm.StatusList2021Credential.md#id)
- [setCredentialStatus](identity_wasm.StatusList2021Credential.md#setcredentialstatus)
- [purpose](identity_wasm.StatusList2021Credential.md#purpose)
- [entry](identity_wasm.StatusList2021Credential.md#entry)
- [clone](identity_wasm.StatusList2021Credential.md#clone)
- [fromJSON](identity_wasm.StatusList2021Credential.md#fromjson)

## Constructors

### constructor

• **new StatusList2021Credential**(`credential`)

Creates a new [StatusList2021Credential](identity_wasm.StatusList2021Credential.md).

#### Parameters

| Name | Type |
| :------ | :------ |
| `credential` | [`Credential`](identity_wasm.Credential.md) |

## Methods

### toJSON

▸ **toJSON**(): `Object`

* Return copy of self without private attributes.

#### Returns

`Object`

▸ **toJSON**(): `any`

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

#### Returns

`string`

___

### setCredentialStatus

▸ **setCredentialStatus**(`credential`, `index`, `revoked_or_suspended`): [`StatusList2021Entry`](identity_wasm.StatusList2021Entry.md)

Sets the given credential's status using the `index`-th entry of this status list.
Returns the created `credentialStatus`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `credential` | [`Credential`](identity_wasm.Credential.md) |
| `index` | `number` |
| `revoked_or_suspended` | `boolean` |

#### Returns

[`StatusList2021Entry`](identity_wasm.StatusList2021Entry.md)

___

### purpose

▸ **purpose**(): [`StatusPurpose`](../enums/identity_wasm.StatusPurpose.md)

Returns the [StatusPurpose](../enums/identity_wasm.StatusPurpose.md) of this [StatusList2021Credential](identity_wasm.StatusList2021Credential.md).

#### Returns

[`StatusPurpose`](../enums/identity_wasm.StatusPurpose.md)

___

### entry

▸ **entry**(`index`): [`CredentialStatus`](../enums/identity_wasm.CredentialStatus.md)

Returns the state of the `index`-th entry, if any.

#### Parameters

| Name | Type |
| :------ | :------ |
| `index` | `number` |

#### Returns

[`CredentialStatus`](../enums/identity_wasm.CredentialStatus.md)

___

### clone

▸ **clone**(): [`StatusList2021Credential`](identity_wasm.StatusList2021Credential.md)

#### Returns

[`StatusList2021Credential`](identity_wasm.StatusList2021Credential.md)

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`StatusList2021Credential`](identity_wasm.StatusList2021Credential.md)

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`StatusList2021Credential`](identity_wasm.StatusList2021Credential.md)
