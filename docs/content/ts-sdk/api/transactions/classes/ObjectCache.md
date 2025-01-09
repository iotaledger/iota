# Class: ObjectCache

## Constructors

### new ObjectCache()

> **new ObjectCache**(`__namedParameters`): [`ObjectCache`](ObjectCache.md)

#### Parameters

• **\_\_namedParameters**: `ObjectCacheOptions`

#### Returns

[`ObjectCache`](ObjectCache.md)

## Methods

### asPlugin()

> **asPlugin**(): [`TransactionPlugin`](../type-aliases/TransactionPlugin.md)

#### Returns

[`TransactionPlugin`](../type-aliases/TransactionPlugin.md)

---

### clear()

> **clear**(): `Promise`\<`void`\>

#### Returns

`Promise`\<`void`\>

---

### getMoveFunctionDefinition()

> **getMoveFunctionDefinition**(`ref`): `Promise`\<`null` \| `MoveFunctionCacheEntry`\>

#### Parameters

• **ref**

• **ref.package**: `string`

• **ref.module**: `string`

• **ref.function**: `string`

#### Returns

`Promise`\<`null` \| `MoveFunctionCacheEntry`\>

---

### getObjects()

> **getObjects**(`ids`): `Promise`\<(`null` \| `ObjectCacheEntry`)[]\>

#### Parameters

• **ids**: `string`[]

#### Returns

`Promise`\<(`null` \| `ObjectCacheEntry`)[]\>

---

### deleteObjects()

> **deleteObjects**(`ids`): `Promise`\<`void`\>

#### Parameters

• **ids**: `string`[]

#### Returns

`Promise`\<`void`\>

---

### clearOwnedObjects()

> **clearOwnedObjects**(): `Promise`\<`void`\>

#### Returns

`Promise`\<`void`\>

---

### clearCustom()

> **clearCustom**(): `Promise`\<`void`\>

#### Returns

`Promise`\<`void`\>

---

### getCustom()

> **getCustom**\<`T`\>(`key`): `Promise`\<`null` \| `T`\>

#### Type Parameters

• **T**

#### Parameters

• **key**: `string`

#### Returns

`Promise`\<`null` \| `T`\>

---

### setCustom()

> **setCustom**\<`T`\>(`key`, `value`): `Promise`\<`void`\>

#### Type Parameters

• **T**

#### Parameters

• **key**: `string`

• **value**: `T`

#### Returns

`Promise`\<`void`\>

---

### deleteCustom()

> **deleteCustom**(`key`): `Promise`\<`void`\>

#### Parameters

• **key**: `string`

#### Returns

`Promise`\<`void`\>

---

### applyEffects()

> **applyEffects**(`effects`): `Promise`\<`void`\>

#### Parameters

• **effects**

• **effects.$kind**: `"V1"`

• **effects.V1** = `TransactionEffectsV1`

• **effects.V1.status**: `EnumOutputShapeWithKeys`\<`object`, `"Success"` \| `"Failed"`\> = `ExecutionStatus`

• **effects.V1.executedEpoch**: `string` = `...`

• **effects.V1.gasUsed** = `GasCostSummary`

• **effects.V1.gasUsed.computationCost**: `string` = `...`

• **effects.V1.gasUsed.computationCostBurned**: `string` = `...`

• **effects.V1.gasUsed.storageCost**: `string` = `...`

• **effects.V1.gasUsed.storageRebate**: `string` = `...`

• **effects.V1.gasUsed.nonRefundableStorageFee**: `string` = `...`

• **effects.V1.transactionDigest**: `string` = `ObjectDigest`

• **effects.V1.gasObjectIndex**: `null` \| `number` = `...`

• **effects.V1.eventsDigest**: `null` \| `string` = `...`

• **effects.V1.dependencies**: `string`[] = `...`

• **effects.V1.lamportVersion**: `string` = `...`

• **effects.V1.changedObjects**: [`string`, `object`][] = `...`

• **effects.V1.unchangedSharedObjects**: [`string`, `EnumOutputShapeWithKeys`\<`object`, `"ReadOnlyRoot"` \| `"MutateDeleted"` \| `"ReadDeleted"` \| `"Cancelled"` \| `"PerEpochConfig"`\>][] = `...`

• **effects.V1.auxDataDigest**: `null` \| `string` = `...`

#### Returns

`Promise`\<`void`\>
