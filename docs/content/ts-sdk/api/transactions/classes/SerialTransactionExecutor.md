# Class: SerialTransactionExecutor

## Constructors

### new SerialTransactionExecutor()

> **new SerialTransactionExecutor**(`__namedParameters`): [`SerialTransactionExecutor`](SerialTransactionExecutor.md)

#### Parameters

• **\_\_namedParameters**: `Omit`\<`ObjectCacheOptions`, `"address"`\> & `object`

#### Returns

[`SerialTransactionExecutor`](SerialTransactionExecutor.md)

## Methods

### applyEffects()

> **applyEffects**(`effects`): `Promise`\<[`void`, `void`]\>

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

`Promise`\<[`void`, `void`]\>

---

### buildTransaction()

> **buildTransaction**(`transaction`): `Promise`\<`Uint8Array`\>

#### Parameters

• **transaction**: [`Transaction`](Transaction.md)

#### Returns

`Promise`\<`Uint8Array`\>

---

### resetCache()

> **resetCache**(): `Promise`\<`void`\>

#### Returns

`Promise`\<`void`\>

---

### waitForLastTransaction()

> **waitForLastTransaction**(): `Promise`\<`void`\>

#### Returns

`Promise`\<`void`\>

---

### executeTransaction()

> **executeTransaction**(`transaction`, `options`?): `Promise`\<`object`\>

#### Parameters

• **transaction**: `Uint8Array` \| [`Transaction`](Transaction.md)

• **options?**: [`IotaTransactionBlockResponseOptions`](../../client/interfaces/IotaTransactionBlockResponseOptions.md)

#### Returns

`Promise`\<`object`\>

##### digest

> **digest**: `string` = `results.digest`

##### effects

> **effects**: `string`
