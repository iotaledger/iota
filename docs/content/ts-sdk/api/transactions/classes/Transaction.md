# Class: Transaction

Transaction Builder

## Constructors

### new Transaction()

> **new Transaction**(): [`Transaction`](Transaction.md)

#### Returns

[`Transaction`](Transaction.md)

## Properties

### object()

> **object**: (`value`) => `object`

Add a new object input to the transaction.

#### Parameters

• **value**: [`TransactionObjectInput`](../type-aliases/TransactionObjectInput.md)

#### Returns

`object`

##### $kind

> **$kind**: `"Input"`

##### Input

> **Input**: `number`

##### type?

> `optional` **type**: `"object"`

#### system()

##### Returns

`object`

###### $kind

> **$kind**: `"Input"`

###### Input

> **Input**: `number`

###### type?

> `optional` **type**: `"object"`

#### clock()

##### Returns

`object`

###### $kind

> **$kind**: `"Input"`

###### Input

> **Input**: `number`

###### type?

> `optional` **type**: `"object"`

#### random()

##### Returns

`object`

###### $kind

> **$kind**: `"Input"`

###### Input

> **Input**: `number`

###### type?

> `optional` **type**: `"object"`

#### denyList()

##### Returns

`object`

###### $kind

> **$kind**: `"Input"`

###### Input

> **Input**: `number`

###### type?

> `optional` **type**: `"object"`

## Accessors

### blockData

#### Get Signature

> **get** **blockData**(): `object`

##### Deprecated

Use `getData()` instead.

##### Returns

`object`

###### ~~version~~

> **version**: `1`

###### ~~sender?~~

> `optional` **sender**: `string`

###### ~~expiration?~~

> `optional` **expiration**: `null` \| `object` \| `object`

###### ~~gasConfig~~

> **gasConfig**: `object` = `GasConfig`

###### gasConfig.budget?

> `optional` **budget**: `string` \| `number` \| `bigint`

###### gasConfig.price?

> `optional` **price**: `string` \| `number` \| `bigint`

###### gasConfig.payment?

> `optional` **payment**: `object`[]

###### gasConfig.owner?

> `optional` **owner**: `string`

###### ~~inputs~~

> **inputs**: (`object` \| `object`)[]

###### ~~transactions~~

> **transactions**: (`object` \| `object` \| `object` \| `object` \| `object` \| `object` \| `object`)[]

---

### pure

#### Get Signature

> **get** **pure**(): \<`T`\>(`type`, `value`) => `T`(`value`) => `T`

##### Returns

`Function`

###### Type Parameters

• **T** _extends_ `PureTypeName`

###### Parameters

• **type**: `T` _extends_ `PureTypeName` ? `ValidPureTypeName`\<`T`\<`T`\>\> : `T`

• **value**: `ShapeFromPureTypeName`\<`T`\>

###### Returns

`T`

###### Parameters

• **value**: `Uint8Array` \| `SerializedBcs`\<`any`, `any`\>

The pure value, serialized to BCS. If this is a Uint8Array, then the value
is assumed to be raw bytes, and will be used directly.

###### Returns

`T`

###### u8()

###### Parameters

• **value**: `number`

###### Returns

`object` \| `object` \| `object` \| `object` \| `object`

###### u16()

###### Parameters

• **value**: `number`

###### Returns

`object` \| `object` \| `object` \| `object` \| `object`

###### u32()

###### Parameters

• **value**: `number`

###### Returns

`object` \| `object` \| `object` \| `object` \| `object`

###### u64()

###### Parameters

• **value**: `string` \| `number` \| `bigint`

###### Returns

`object` \| `object` \| `object` \| `object` \| `object`

###### u128()

###### Parameters

• **value**: `string` \| `number` \| `bigint`

###### Returns

`object` \| `object` \| `object` \| `object` \| `object`

###### u256()

###### Parameters

• **value**: `string` \| `number` \| `bigint`

###### Returns

`object` \| `object` \| `object` \| `object` \| `object`

###### bool()

###### Parameters

• **value**: `boolean`

###### Returns

`object` \| `object` \| `object` \| `object` \| `object`

###### string()

###### Parameters

• **value**: `string`

###### Returns

`object` \| `object` \| `object` \| `object` \| `object`

###### address()

###### Parameters

• **value**: `string`

###### Returns

`object` \| `object` \| `object` \| `object` \| `object`

###### id()

> **id**: (`value`) => `object` \| `object` \| `object` \| `object` \| `object`

###### Parameters

• **value**: `string`

###### Returns

`object` \| `object` \| `object` \| `object` \| `object`

###### vector()

###### Type Parameters

• **Type** _extends_ `PureTypeName`

###### Parameters

• **type**: `Type`

• **value**: `Iterable`\<`ShapeFromPureTypeName`\<`Type`\>, `any`, `any`\> & `object`

###### Returns

`object` \| `object` \| `object` \| `object` \| `object`

###### option()

###### Type Parameters

• **Type** _extends_ `PureTypeName`

###### Parameters

• **type**: `Type`

• **value**: `undefined` \| `null` \| `ShapeFromPureTypeName`\<`Type`\>

###### Returns

`object` \| `object` \| `object` \| `object` \| `object`

---

### gas

#### Get Signature

> **get** **gas**(): `object`

Returns an argument for the gas coin, to be used in a transaction.

##### Returns

`object`

###### $kind

> **$kind**: `"GasCoin"`

###### GasCoin

> **GasCoin**: `true`

## Methods

### fromKind()

> `static` **fromKind**(`serialized`): [`Transaction`](Transaction.md)

Converts from a serialize transaction kind (built with `build({ onlyTransactionKind: true })`) to a `Transaction` class.
Supports either a byte array, or base64-encoded bytes.

#### Parameters

• **serialized**: `string` \| `Uint8Array`

#### Returns

[`Transaction`](Transaction.md)

---

### from()

> `static` **from**(`transaction`): [`Transaction`](Transaction.md)

Converts from a serialized transaction format to a `Transaction` class.
There are two supported serialized formats:

- A string returned from `Transaction#serialize`. The serialized format must be compatible, or it will throw an error.
- A byte array (or base64-encoded bytes) containing BCS transaction data.

#### Parameters

• **transaction**: `string` \| `Uint8Array` \| [`Transaction`](Transaction.md)

#### Returns

[`Transaction`](Transaction.md)

---

### registerGlobalSerializationPlugin()

> `static` **registerGlobalSerializationPlugin**(`step`): `void`

#### Parameters

• **step**: [`TransactionPlugin`](../type-aliases/TransactionPlugin.md)

#### Returns

`void`

---

### registerGlobalBuildPlugin()

> `static` **registerGlobalBuildPlugin**(`step`): `void`

#### Parameters

• **step**: [`TransactionPlugin`](../type-aliases/TransactionPlugin.md)

#### Returns

`void`

---

### addSerializationPlugin()

> **addSerializationPlugin**(`step`): `void`

#### Parameters

• **step**: [`TransactionPlugin`](../type-aliases/TransactionPlugin.md)

#### Returns

`void`

---

### addBuildPlugin()

> **addBuildPlugin**(`step`): `void`

#### Parameters

• **step**: [`TransactionPlugin`](../type-aliases/TransactionPlugin.md)

#### Returns

`void`

---

### addIntentResolver()

> **addIntentResolver**(`intent`, `resolver`): `void`

#### Parameters

• **intent**: `string`

• **resolver**: [`TransactionPlugin`](../type-aliases/TransactionPlugin.md)

#### Returns

`void`

---

### setSender()

> **setSender**(`sender`): `void`

#### Parameters

• **sender**: `string`

#### Returns

`void`

---

### setSenderIfNotSet()

> **setSenderIfNotSet**(`sender`): `void`

Sets the sender only if it has not already been set.
This is useful for sponsored transaction flows where the sender may not be the same as the signer address.

#### Parameters

• **sender**: `string`

#### Returns

`void`

---

### setExpiration()

> **setExpiration**(`expiration`?): `void`

#### Parameters

• **expiration?**: `null` \| `EnumInputShape`\<`object`\>

#### Returns

`void`

---

### setGasPrice()

> **setGasPrice**(`price`): `void`

#### Parameters

• **price**: `number` \| `bigint`

#### Returns

`void`

---

### setGasBudget()

> **setGasBudget**(`budget`): `void`

#### Parameters

• **budget**: `number` \| `bigint`

#### Returns

`void`

---

### setGasBudgetIfNotSet()

> **setGasBudgetIfNotSet**(`budget`): `void`

#### Parameters

• **budget**: `number` \| `bigint`

#### Returns

`void`

---

### setGasOwner()

> **setGasOwner**(`owner`): `void`

#### Parameters

• **owner**: `string`

#### Returns

`void`

---

### setGasPayment()

> **setGasPayment**(`payments`): `void`

#### Parameters

• **payments**: `object`[]

#### Returns

`void`

---

### getData()

> **getData**(): `object`

Get a snapshot of the transaction data, in JSON form:

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

---

### objectRef()

> **objectRef**(...`args`): `object`

Add a new object input to the transaction using the fully-resolved object reference.
If you only have an object ID, use `builder.object(id)` instead.

#### Parameters

• ...**args**: [`object`]

#### Returns

`object`

##### $kind

> **$kind**: `"Input"`

##### Input

> **Input**: `number`

##### type?

> `optional` **type**: `"object"`

---

### receivingRef()

> **receivingRef**(...`args`): `object`

Add a new receiving input to the transaction using the fully-resolved object reference.
If you only have an object ID, use `builder.object(id)` instead.

#### Parameters

• ...**args**: [`object`]

#### Returns

`object`

##### $kind

> **$kind**: `"Input"`

##### Input

> **Input**: `number`

##### type?

> `optional` **type**: `"object"`

---

### sharedObjectRef()

> **sharedObjectRef**(...`args`): `object`

Add a new shared object input to the transaction using the fully-resolved shared object reference.
If you only have an object ID, use `builder.object(id)` instead.

#### Parameters

• ...**args**: [`object`]

#### Returns

`object`

##### $kind

> **$kind**: `"Input"`

##### Input

> **Input**: `number`

##### type?

> `optional` **type**: `"object"`

---

### add()

> **add**\<`T`\>(`command`): `T`

Add a transaction to the transaction

#### Type Parameters

• **T** = [`TransactionResult`](../type-aliases/TransactionResult.md)

#### Parameters

• **command**: [`Command`](../type-aliases/Command.md) \| (`tx`) => `T`

#### Returns

`T`

---

### splitCoins()

> **splitCoins**(`coin`, `amounts`): [`TransactionResult`](../type-aliases/TransactionResult.md)

#### Parameters

• **coin**: `string` \| [`TransactionObjectArgument`](../type-aliases/TransactionObjectArgument.md)

• **amounts**: (`string` \| `number` \| `bigint` \| `SerializedBcs`\<`any`, `any`\> \| [`TransactionArgument`](../type-aliases/TransactionArgument.md))[]

#### Returns

[`TransactionResult`](../type-aliases/TransactionResult.md)

---

### mergeCoins()

> **mergeCoins**(`destination`, `sources`): [`TransactionResult`](../type-aliases/TransactionResult.md)

#### Parameters

• **destination**: `string` \| [`TransactionObjectArgument`](../type-aliases/TransactionObjectArgument.md)

• **sources**: (`string` \| [`TransactionObjectArgument`](../type-aliases/TransactionObjectArgument.md))[]

#### Returns

[`TransactionResult`](../type-aliases/TransactionResult.md)

---

### publish()

> **publish**(`__namedParameters`): [`TransactionResult`](../type-aliases/TransactionResult.md)

#### Parameters

• **\_\_namedParameters**

• **\_\_namedParameters.modules**: `string`[] \| `number`[][]

• **\_\_namedParameters.dependencies**: `string`[]

#### Returns

[`TransactionResult`](../type-aliases/TransactionResult.md)

---

### upgrade()

> **upgrade**(`__namedParameters`): [`TransactionResult`](../type-aliases/TransactionResult.md)

#### Parameters

• **\_\_namedParameters**

• **\_\_namedParameters.modules**: `string`[] \| `number`[][]

• **\_\_namedParameters.dependencies**: `string`[]

• **\_\_namedParameters.package**: `string`

• **\_\_namedParameters.ticket**: `string` \| [`TransactionObjectArgument`](../type-aliases/TransactionObjectArgument.md)

#### Returns

[`TransactionResult`](../type-aliases/TransactionResult.md)

---

### moveCall()

> **moveCall**(`__namedParameters`): [`TransactionResult`](../type-aliases/TransactionResult.md)

#### Parameters

• **\_\_namedParameters**: `object` \| `object`

#### Returns

[`TransactionResult`](../type-aliases/TransactionResult.md)

---

### transferObjects()

> **transferObjects**(`objects`, `address`): [`TransactionResult`](../type-aliases/TransactionResult.md)

#### Parameters

• **objects**: (`string` \| [`TransactionObjectArgument`](../type-aliases/TransactionObjectArgument.md))[]

• **address**: `string` \| `SerializedBcs`\<`any`, `any`\> \| [`TransactionArgument`](../type-aliases/TransactionArgument.md)

#### Returns

[`TransactionResult`](../type-aliases/TransactionResult.md)

---

### makeMoveVec()

> **makeMoveVec**(`__namedParameters`): [`TransactionResult`](../type-aliases/TransactionResult.md)

#### Parameters

• **\_\_namedParameters**

• **\_\_namedParameters.elements**: (`string` \| [`TransactionObjectArgument`](../type-aliases/TransactionObjectArgument.md))[]

• **\_\_namedParameters.type?**: `string`

#### Returns

[`TransactionResult`](../type-aliases/TransactionResult.md)

---

### ~~serialize()~~

> **serialize**(): `string`

#### Returns

`string`

#### Deprecated

Use toJSON instead.
For synchronous serialization, you can use `getData()`

---

### toJSON()

> **toJSON**(`options`): `Promise`\<`string`\>

#### Parameters

• **options**: [`SerializeTransactionOptions`](../interfaces/SerializeTransactionOptions.md) = `{}`

#### Returns

`Promise`\<`string`\>

---

### sign()

> **sign**(`options`): `Promise`\<[`SignatureWithBytes`](../../cryptography/interfaces/SignatureWithBytes.md)\>

Build the transaction to BCS bytes, and sign it with the provided keypair.

#### Parameters

• **options**: `SignOptions`

#### Returns

`Promise`\<[`SignatureWithBytes`](../../cryptography/interfaces/SignatureWithBytes.md)\>

---

### build()

> **build**(`options`): `Promise`\<`Uint8Array`\>

Build the transaction to BCS bytes.

#### Parameters

• **options**: [`BuildTransactionOptions`](../interfaces/BuildTransactionOptions.md) = `{}`

#### Returns

`Promise`\<`Uint8Array`\>

---

### getDigest()

> **getDigest**(`options`): `Promise`\<`string`\>

Derive transaction digest

#### Parameters

• **options** = `{}`

• **options.client?**: [`IotaClient`](../../client/classes/IotaClient.md)

#### Returns

`Promise`\<`string`\>

---

### prepareForSerialization()

> **prepareForSerialization**(`options`): `Promise`\<`void`\>

#### Parameters

• **options**: [`SerializeTransactionOptions`](../interfaces/SerializeTransactionOptions.md)

#### Returns

`Promise`\<`void`\>
