# Class: ParallelTransactionExecutor

## Constructors

### new ParallelTransactionExecutor()

> **new ParallelTransactionExecutor**(`options`): [`ParallelTransactionExecutor`](ParallelTransactionExecutor.md)

#### Parameters

• **options**: [`ParallelTransactionExecutorOptions`](../interfaces/ParallelTransactionExecutorOptions.md)

#### Returns

[`ParallelTransactionExecutor`](ParallelTransactionExecutor.md)

## Methods

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

• **transaction**: [`Transaction`](Transaction.md)

• **options?**: [`IotaTransactionBlockResponseOptions`](../../client/interfaces/IotaTransactionBlockResponseOptions.md)

#### Returns

`Promise`\<`object`\>

##### digest

> **digest**: `string`

##### effects

> **effects**: `string`
