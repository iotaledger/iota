# Class: TransactionDataBuilder

## Implements

- [`TransactionData`](../type-aliases/TransactionData.md)

## Constructors

### new TransactionDataBuilder()

> **new TransactionDataBuilder**(`clone`?): [`TransactionDataBuilder`](TransactionDataBuilder.md)

#### Parameters

• **clone?**

• **clone.version?**: `2` = `...`

• **clone.sender?**: `null` \| `string` = `...`

• **clone.expiration?**: `null` \| `EnumOutputShapeWithKeys`\<`object`, `"None"` \| `"Epoch"`\> = `...`

• **clone.gasData?** = `GasData`

• **clone.gasData.budget?**: `null` \| `string` \| `number` = `...`

• **clone.gasData.price?**: `null` \| `string` \| `number` = `...`

• **clone.gasData.owner?**: `null` \| `string` = `...`

• **clone.gasData.payment?**: `null` \| `object`[] = `...`

• **clone.inputs?**: `EnumOutputShapeWithKeys`\<`object`, `"Pure"` \| `"Object"` \| `"UnresolvedPure"` \| `"UnresolvedObject"`\>[] = `...`

• **clone.commands?**: `EnumOutputShapeWithKeys`\<`object`, `"MoveCall"` \| `"TransferObjects"` \| `"SplitCoins"` \| `"MergeCoins"` \| `"Publish"` \| `"MakeMoveVec"` \| `"Upgrade"` \| `"$Intent"`\>[] = `...`

#### Returns

[`TransactionDataBuilder`](TransactionDataBuilder.md)

## Properties

### version

> **version**: `2`

#### Implementation of

`TransactionData.version`

---

### sender

> **sender**: `null` \| `string`

#### Implementation of

`TransactionData.sender`

---

### expiration

> **expiration**: `null` \| `EnumOutputShapeWithKeys`\<`object`, `"None"` \| `"Epoch"`\>

#### Implementation of

`TransactionData.expiration`

---

### gasData

> **gasData**: `object`

#### budget

> **budget**: `null` \| `string` \| `number`

#### price

> **price**: `null` \| `string` \| `number`

#### owner

> **owner**: `null` \| `string`

#### payment

> **payment**: `null` \| `object`[]

#### Implementation of

`TransactionData.gasData`

---

### inputs

> **inputs**: `EnumOutputShapeWithKeys`\<`object`, `"Pure"` \| `"Object"` \| `"UnresolvedPure"` \| `"UnresolvedObject"`\>[]

#### Implementation of

`TransactionData.inputs`

---

### commands

> **commands**: [`Command`](../type-aliases/Command.md)[]

#### Implementation of

`TransactionData.commands`

## Accessors

### gasConfig

#### Get Signature

> **get** **gasConfig**(): `object`

##### Returns

`object`

###### budget

> **budget**: `null` \| `string` \| `number`

###### price

> **price**: `null` \| `string` \| `number`

###### owner

> **owner**: `null` \| `string`

###### payment

> **payment**: `null` \| `object`[]

#### Set Signature

> **set** **gasConfig**(`value`): `void`

##### Parameters

• **value**

• **value.budget**: `null` \| `string` \| `number` = `...`

• **value.price**: `null` \| `string` \| `number` = `...`

• **value.owner**: `null` \| `string` = `...`

• **value.payment**: `null` \| `object`[] = `...`

##### Returns

`void`

## Methods

### fromKindBytes()

> `static` **fromKindBytes**(`bytes`): [`TransactionDataBuilder`](TransactionDataBuilder.md)

#### Parameters

• **bytes**: `Uint8Array`

#### Returns

[`TransactionDataBuilder`](TransactionDataBuilder.md)

---

### fromBytes()

> `static` **fromBytes**(`bytes`): [`TransactionDataBuilder`](TransactionDataBuilder.md)

#### Parameters

• **bytes**: `Uint8Array`

#### Returns

[`TransactionDataBuilder`](TransactionDataBuilder.md)

---

### restore()

> `static` **restore**(`data`): [`TransactionDataBuilder`](TransactionDataBuilder.md)

#### Parameters

• **data**: `object` \| `object`

#### Returns

[`TransactionDataBuilder`](TransactionDataBuilder.md)

---

### getDigestFromBytes()

> `static` **getDigestFromBytes**(`bytes`): `string`

Generate transaction digest.

#### Parameters

• **bytes**: `Uint8Array`

BCS serialized transaction data

#### Returns

`string`

transaction digest.

---

### build()

> **build**(`__namedParameters`): `Uint8Array`

#### Parameters

• **\_\_namedParameters** = `{}`

• **\_\_namedParameters.maxSizeBytes?**: `number` = `Infinity`

• **\_\_namedParameters.overrides?**

• **\_\_namedParameters.overrides.expiration?**: `EnumOutputShapeWithKeys`\<`object`, `"None"` \| `"Epoch"`\>

• **\_\_namedParameters.overrides.sender?**: `string`

• **\_\_namedParameters.overrides.gasConfig?**: `Partial`\<`object`\>

• **\_\_namedParameters.overrides.gasData?**: `Partial`\<`object`\>

• **\_\_namedParameters.onlyTransactionKind?**: `boolean`

#### Returns

`Uint8Array`

---

### addInput()

> **addInput**\<`T`\>(`type`, `arg`): `object`

#### Type Parameters

• **T** _extends_ `"object"` \| `"pure"`

#### Parameters

• **type**: `T`

• **arg**: `EnumOutputShapeWithKeys`\<`object`, `"Pure"` \| `"Object"` \| `"UnresolvedPure"` \| `"UnresolvedObject"`\>

#### Returns

`object`

##### Input

> **Input**: `number` = `index`

##### type

> **type**: `T`

##### $kind

> **$kind**: `"Input"`

---

### getInputUses()

> **getInputUses**(`index`, `fn`): `void`

#### Parameters

• **index**: `number`

• **fn**

#### Returns

`void`

---

### mapArguments()

> **mapArguments**(`fn`): `void`

#### Parameters

• **fn**

#### Returns

`void`

---

### replaceCommand()

> **replaceCommand**(`index`, `replacement`): `void`

#### Parameters

• **index**: `number`

• **replacement**: [`Command`](../type-aliases/Command.md) \| [`Command`](../type-aliases/Command.md)[]

#### Returns

`void`

---

### getDigest()

> **getDigest**(): `string`

#### Returns

`string`

---

### snapshot()

> **snapshot**(): `object`

#### Returns

`object`

##### version

> **version**: `2`

##### sender?

> `optional` **sender**: `null` \| `string`

##### expiration?

> `optional` **expiration**: `null` \| `EnumOutputShapeWithKeys`\<`object`, `"None"` \| `"Epoch"`\>

##### gasData

> **gasData**: `object` = `GasData`

##### gasData.budget

> **budget**: `null` \| `string` \| `number`

##### gasData.price

> **price**: `null` \| `string` \| `number`

##### gasData.owner

> **owner**: `null` \| `string`

##### gasData.payment

> **payment**: `null` \| `object`[]

##### inputs

> **inputs**: `EnumOutputShapeWithKeys`\<`object`, `"Pure"` \| `"Object"` \| `"UnresolvedPure"` \| `"UnresolvedObject"`\>[]

##### commands

> **commands**: `EnumOutputShapeWithKeys`\<`object`, `"MoveCall"` \| `"TransferObjects"` \| `"SplitCoins"` \| `"MergeCoins"` \| `"Publish"` \| `"MakeMoveVec"` \| `"Upgrade"` \| `"$Intent"`\>[]
