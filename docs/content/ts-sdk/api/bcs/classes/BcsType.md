# Class: BcsType\<T, Input\>

## Type Parameters

- **T**
- **Input** = `T`

## Constructors

### new BcsType()

> **new BcsType**\<`T`, `Input`\>(`options`): [`BcsType`](BcsType.md)\<`T`, `Input`\>

#### Parameters

- **options**: `object` & [`BcsTypeOptions`](../interfaces/BcsTypeOptions.md)\<`T`, `Input`\>

#### Returns

[`BcsType`](BcsType.md)\<`T`, `Input`\>

## Properties

### $inferType

> **$inferType**: `T`

---

### $inferInput

> **$inferInput**: `Input`

---

### name

> **name**: `string`

---

### read()

> **read**: (`reader`) => `T`

#### Parameters

- **reader**: `BcsReader`

#### Returns

`T`

---

### serializedSize()

> **serializedSize**: (`value`, `options`?) => `null` \| `number`

#### Parameters

- **value**: `Input`
- **options?**: `BcsWriterOptions`

#### Returns

`null` \| `number`

---

### validate()

> **validate**: (`value`) => `void`

#### Parameters

- **value**: `Input`

#### Returns

`void`

## Methods

### write()

> **write**(`value`, `writer`): `void`

#### Parameters

- **value**: `Input`
- **writer**: `BcsWriter`

#### Returns

`void`

---

### serialize()

> **serialize**(`value`, `options`?): `SerializedBcs`\<`T`, `Input`\>

#### Parameters

- **value**: `Input`
- **options?**: `BcsWriterOptions`

#### Returns

`SerializedBcs`\<`T`, `Input`\>

---

### parse()

> **parse**(`bytes`): `T`

#### Parameters

- **bytes**: `Uint8Array`

#### Returns

`T`

---

### fromHex()

> **fromHex**(`hex`): `T`

#### Parameters

- **hex**: `string`

#### Returns

`T`

---

### fromBase58()

> **fromBase58**(`b64`): `T`

#### Parameters

- **b64**: `string`

#### Returns

`T`

---

### fromBase64()

> **fromBase64**(`b64`): `T`

#### Parameters

- **b64**: `string`

#### Returns

`T`

---

### transform()

> **transform**\<`T2`, `Input2`\>(`__namedParameters`): [`BcsType`](BcsType.md)\<`T2`, `Input2`\>

#### Type Parameters

- **T2**
- **Input2**

#### Parameters

- **\_\_namedParameters**: `object` & [`BcsTypeOptions`](../interfaces/BcsTypeOptions.md)\<`T2`, `Input2`\>

#### Returns

[`BcsType`](BcsType.md)\<`T2`, `Input2`\>
