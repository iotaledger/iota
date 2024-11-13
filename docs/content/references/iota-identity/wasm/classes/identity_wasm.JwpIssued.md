# Class: JwpIssued

[identity\_wasm](../modules/identity_wasm.md).JwpIssued

## Table of contents

### Methods

- [toJSON](identity_wasm.JwpIssued.md#tojson)
- [fromJSON](identity_wasm.JwpIssued.md#fromjson)
- [clone](identity_wasm.JwpIssued.md#clone)
- [encode](identity_wasm.JwpIssued.md#encode)
- [setProof](identity_wasm.JwpIssued.md#setproof)
- [getProof](identity_wasm.JwpIssued.md#getproof)
- [getPayloads](identity_wasm.JwpIssued.md#getpayloads)
- [setPayloads](identity_wasm.JwpIssued.md#setpayloads)
- [getIssuerProtectedHeader](identity_wasm.JwpIssued.md#getissuerprotectedheader)

## Methods

### toJSON

▸ **toJSON**(): `any`

Serializes this to a JSON object.

#### Returns

`any`

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`JwpIssued`](identity_wasm.JwpIssued.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`JwpIssued`](identity_wasm.JwpIssued.md)

___

### clone

▸ **clone**(): [`JwpIssued`](identity_wasm.JwpIssued.md)

Deep clones the object.

#### Returns

[`JwpIssued`](identity_wasm.JwpIssued.md)

___

### encode

▸ **encode**(`serialization`): `string`

#### Parameters

| Name | Type |
| :------ | :------ |
| `serialization` | [`SerializationType`](../enums/identity_wasm.SerializationType.md) |

#### Returns

`string`

___

### setProof

▸ **setProof**(`proof`): `void`

#### Parameters

| Name | Type |
| :------ | :------ |
| `proof` | `Uint8Array` |

#### Returns

`void`

___

### getProof

▸ **getProof**(): `Uint8Array`

#### Returns

`Uint8Array`

___

### getPayloads

▸ **getPayloads**(): [`Payloads`](identity_wasm.Payloads.md)

#### Returns

[`Payloads`](identity_wasm.Payloads.md)

___

### setPayloads

▸ **setPayloads**(`payloads`): `void`

#### Parameters

| Name | Type |
| :------ | :------ |
| `payloads` | [`Payloads`](identity_wasm.Payloads.md) |

#### Returns

`void`

___

### getIssuerProtectedHeader

▸ **getIssuerProtectedHeader**(): [`IssuerProtectedHeader`](identity_wasm.IssuerProtectedHeader.md)

#### Returns

[`IssuerProtectedHeader`](identity_wasm.IssuerProtectedHeader.md)
