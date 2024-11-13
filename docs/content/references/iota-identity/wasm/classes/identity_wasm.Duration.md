# Class: Duration

[identity\_wasm](../modules/identity_wasm.md).Duration

A span of time.

## Table of contents

### Methods

- [toJSON](identity_wasm.Duration.md#tojson)
- [toString](identity_wasm.Duration.md#tostring)
- [seconds](identity_wasm.Duration.md#seconds)
- [minutes](identity_wasm.Duration.md#minutes)
- [hours](identity_wasm.Duration.md#hours)
- [days](identity_wasm.Duration.md#days)
- [weeks](identity_wasm.Duration.md#weeks)
- [fromJSON](identity_wasm.Duration.md#fromjson)

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

### seconds

▸ `Static` **seconds**(`seconds`): [`Duration`](identity_wasm.Duration.md)

Create a new [Duration](identity_wasm.Duration.md) with the given number of seconds.

#### Parameters

| Name | Type |
| :------ | :------ |
| `seconds` | `number` |

#### Returns

[`Duration`](identity_wasm.Duration.md)

___

### minutes

▸ `Static` **minutes**(`minutes`): [`Duration`](identity_wasm.Duration.md)

Create a new [Duration](identity_wasm.Duration.md) with the given number of minutes.

#### Parameters

| Name | Type |
| :------ | :------ |
| `minutes` | `number` |

#### Returns

[`Duration`](identity_wasm.Duration.md)

___

### hours

▸ `Static` **hours**(`hours`): [`Duration`](identity_wasm.Duration.md)

Create a new [Duration](identity_wasm.Duration.md) with the given number of hours.

#### Parameters

| Name | Type |
| :------ | :------ |
| `hours` | `number` |

#### Returns

[`Duration`](identity_wasm.Duration.md)

___

### days

▸ `Static` **days**(`days`): [`Duration`](identity_wasm.Duration.md)

Create a new [Duration](identity_wasm.Duration.md) with the given number of days.

#### Parameters

| Name | Type |
| :------ | :------ |
| `days` | `number` |

#### Returns

[`Duration`](identity_wasm.Duration.md)

___

### weeks

▸ `Static` **weeks**(`weeks`): [`Duration`](identity_wasm.Duration.md)

Create a new [Duration](identity_wasm.Duration.md) with the given number of weeks.

#### Parameters

| Name | Type |
| :------ | :------ |
| `weeks` | `number` |

#### Returns

[`Duration`](identity_wasm.Duration.md)

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`Duration`](identity_wasm.Duration.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`Duration`](identity_wasm.Duration.md)
