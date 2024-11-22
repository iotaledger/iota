# Variable: bcs

> `const` **bcs**: `object`

## Type declaration

### u8()

Creates a BcsType that can be used to read and write an 8-bit unsigned integer.

#### Parameters

• **options?**: [`BcsTypeOptions`](../interfaces/BcsTypeOptions.md)\<`number`, `number`\>

#### Returns

[`BcsType`](../classes/BcsType.md)\<`number`, `number`\>

#### Example

```ts
bcs.u8().serialize(255).toBytes() // Uint8Array [ 255 ]
```

### u16()

Creates a BcsType that can be used to read and write a 16-bit unsigned integer.

#### Parameters

• **options?**: [`BcsTypeOptions`](../interfaces/BcsTypeOptions.md)\<`number`, `number`\>

#### Returns

[`BcsType`](../classes/BcsType.md)\<`number`, `number`\>

#### Example

```ts
bcs.u16().serialize(65535).toBytes() // Uint8Array [ 255, 255 ]
```

### u32()

Creates a BcsType that can be used to read and write a 32-bit unsigned integer.

#### Parameters

• **options?**: [`BcsTypeOptions`](../interfaces/BcsTypeOptions.md)\<`number`, `number`\>

#### Returns

[`BcsType`](../classes/BcsType.md)\<`number`, `number`\>

#### Example

```ts
bcs.u32().serialize(4294967295).toBytes() // Uint8Array [ 255, 255, 255, 255 ]
```

### u64()

Creates a BcsType that can be used to read and write a 64-bit unsigned integer.

#### Parameters

• **options?**: [`BcsTypeOptions`](../interfaces/BcsTypeOptions.md)\<`string`, `string` \| `number` \| `bigint`\>

#### Returns

[`BcsType`](../classes/BcsType.md)\<`string`, `string` \| `number` \| `bigint`\>

#### Example

```ts
bcs.u64().serialize(1).toBytes() // Uint8Array [ 1, 0, 0, 0, 0, 0, 0, 0 ]
```

### u128()

Creates a BcsType that can be used to read and write a 128-bit unsigned integer.

#### Parameters

• **options?**: [`BcsTypeOptions`](../interfaces/BcsTypeOptions.md)\<`string`, `string` \| `number` \| `bigint`\>

#### Returns

[`BcsType`](../classes/BcsType.md)\<`string`, `string` \| `number` \| `bigint`\>

#### Example

```ts
bcs.u128().serialize(1).toBytes() // Uint8Array [ 1, ..., 0 ]
```

### u256()

Creates a BcsType that can be used to read and write a 256-bit unsigned integer.

#### Parameters

• **options?**: [`BcsTypeOptions`](../interfaces/BcsTypeOptions.md)\<`string`, `string` \| `number` \| `bigint`\>

#### Returns

[`BcsType`](../classes/BcsType.md)\<`string`, `string` \| `number` \| `bigint`\>

#### Example

```ts
bcs.u256().serialize(1).toBytes() // Uint8Array [ 1, ..., 0 ]
```

### bool()

Creates a BcsType that can be used to read and write boolean values.

#### Parameters

• **options?**: [`BcsTypeOptions`](../interfaces/BcsTypeOptions.md)\<`boolean`, `boolean`\>

#### Returns

[`BcsType`](../classes/BcsType.md)\<`boolean`, `boolean`\>

#### Example

```ts
bcs.bool().serialize(true).toBytes() // Uint8Array [ 1 ]
```

### uleb128()

Creates a BcsType that can be used to read and write unsigned LEB encoded integers

#### Parameters

• **options?**: [`BcsTypeOptions`](../interfaces/BcsTypeOptions.md)\<`number`, `number`\>

#### Returns

[`BcsType`](../classes/BcsType.md)\<`number`, `number`\>

#### Example

```ts
```

### bytes()

Creates a BcsType representing a fixed length byte array

#### Type Parameters

• **T** _extends_ `number`

#### Parameters

• **size**: `T`

The number of bytes this types represents

• **options?**: [`BcsTypeOptions`](../interfaces/BcsTypeOptions.md)\<`Uint8Array`, `Iterable`\<`number`, `any`, `any`\>\>

#### Returns

[`BcsType`](../classes/BcsType.md)\<`Uint8Array`, `Uint8Array`\>

#### Example

```ts
bcs.bytes(3).serialize(new Uint8Array([1, 2, 3])).toBytes() // Uint8Array [1, 2, 3]
```

### string()

Creates a BcsType that can ser/de string values. Strings will be UTF-8 encoded

#### Parameters

• **options?**: [`BcsTypeOptions`](../interfaces/BcsTypeOptions.md)\<`string`, `string`\>

#### Returns

[`BcsType`](../classes/BcsType.md)\<`string`, `string`\>

#### Example

```ts
bcs.string().serialize('a').toBytes() // Uint8Array [ 1, 97 ]
```

### fixedArray()

Creates a BcsType that represents a fixed length array of a given type

#### Type Parameters

• **T**

• **Input**

#### Parameters

• **size**: `number`

The number of elements in the array

• **type**: [`BcsType`](../classes/BcsType.md)\<`T`, `Input`\>

The BcsType of each element in the array

• **options?**: [`BcsTypeOptions`](../interfaces/BcsTypeOptions.md)\<`T`[], `Iterable`\<`Input`, `any`, `any`\> & `object`\>

#### Returns

[`BcsType`](../classes/BcsType.md)\<`T`[], `Iterable`\<`Input`, `any`, `any`\> & `object`\>

#### Example

```ts
bcs.fixedArray(3, bcs.u8()).serialize([1, 2, 3]).toBytes() // Uint8Array [ 1, 2, 3 ]
```

### option()

Creates a BcsType representing an optional value

#### Type Parameters

• **T**

• **Input**

#### Parameters

• **type**: [`BcsType`](../classes/BcsType.md)\<`T`, `Input`\>

The BcsType of the optional value

#### Returns

[`BcsType`](../classes/BcsType.md)\<`null` \| `T`, `undefined` \| `null` \| `Input`\>

#### Example

```ts
bcs.option(bcs.u8()).serialize(null).toBytes() // Uint8Array [ 0 ]
bcs.option(bcs.u8()).serialize(1).toBytes() // Uint8Array [ 1, 1 ]
```

### vector()

Creates a BcsType representing a variable length vector of a given type

#### Type Parameters

• **T**

• **Input**

#### Parameters

• **type**: [`BcsType`](../classes/BcsType.md)\<`T`, `Input`\>

The BcsType of each element in the vector

• **options?**: [`BcsTypeOptions`](../interfaces/BcsTypeOptions.md)\<`T`[], `Iterable`\<`Input`, `any`, `any`\> & `object`\>

#### Returns

[`BcsType`](../classes/BcsType.md)\<`T`[], `Iterable`\<`Input`, `any`, `any`\> & `object`\>

#### Example

```ts
bcs.vector(bcs.u8()).toBytes([1, 2, 3]) // Uint8Array [ 3, 1, 2, 3 ]
```

### tuple()

Creates a BcsType representing a tuple of a given set of types

#### Type Parameters

• **Types** _extends_ readonly [`BcsType`](../classes/BcsType.md)\<`any`, `any`\>[]

#### Parameters

• **types**: `Types`

The BcsTypes for each element in the tuple

• **options?**: [`BcsTypeOptions`](../interfaces/BcsTypeOptions.md)\<\{ -readonly \[K in string \| number \| symbol\]: Types\[K\<K\>\] extends BcsType\<T, any\> ? T : never \}, \{ \[K in string \| number \| symbol\]: Types\[K\<K\>\] extends BcsType\<any, T\> ? T : never \}\>

#### Returns

[`BcsType`](../classes/BcsType.md)\<\{ -readonly \[K in string \| number \| symbol\]: Types\[K\<K\>\] extends BcsType\<T, any\> ? T : never \}, \{ \[K\_1 in string \| number \| symbol\]: Types\[K\_1\<K\_1\>\] extends BcsType\<any, T\_1\> ? T\_1 : never \}\>

#### Example

```ts
const tuple = bcs.tuple([bcs.u8(), bcs.string(), bcs.bool()])
tuple.serialize([1, 'a', true]).toBytes() // Uint8Array [ 1, 1, 97, 1 ]
```

### struct()

Creates a BcsType representing a struct of a given set of fields

#### Type Parameters

• **T** _extends_ `Record`\<`string`, [`BcsType`](../classes/BcsType.md)\<`any`, `any`\>\>

#### Parameters

• **name**: `string`

The name of the struct

• **fields**: `T`

The fields of the struct. The order of the fields affects how data is serialized and deserialized

• **options?**: `Omit`\<[`BcsTypeOptions`](../interfaces/BcsTypeOptions.md)\<\{ \[K in string \| number \| symbol\]: T\[K\] extends BcsType\<U, any\> ? U : never \}, \{ \[K in string \| number \| symbol\]: T\[K\] extends BcsType\<any, U\> ? U : never \}\>, `"name"`\>

#### Returns

[`BcsType`](../classes/BcsType.md)\<\{ \[K in string \| number \| symbol\]: T\[K\] extends BcsType\<U, any\> ? U : never \}, \{ \[K\_1 in string \| number \| symbol\]: T\[K\_1\] extends BcsType\<any, U\_1\> ? U\_1 : never \}\>

#### Example

```ts
const struct = bcs.struct('MyStruct', {
 a: bcs.u8(),
 b: bcs.string(),
})
struct.serialize({ a: 1, b: 'a' }).toBytes() // Uint8Array [ 1, 1, 97 ]
```

### enum()

Creates a BcsType representing an enum of a given set of options

#### Type Parameters

• **T** _extends_ `Record`\<`string`, `null` \| [`BcsType`](../classes/BcsType.md)\<`any`, `any`\>\>

#### Parameters

• **name**: `string`

The name of the enum

• **values**: `T`

The values of the enum. The order of the values affects how data is serialized and deserialized.
null can be used to represent a variant with no data.

• **options?**: `Omit`\<[`BcsTypeOptions`](../interfaces/BcsTypeOptions.md)\<`EnumOutputShape`\<\{ \[K in string \| number \| symbol\]: T\[K\] extends BcsType\<U, any\> ? U : true \}\>, `EnumInputShape`\<\{ \[K in string \| number \| symbol\]: T\[K\] extends BcsType\<any, U\> ? U : null \| boolean \| object \}\>\>, `"name"`\>

#### Returns

[`BcsType`](../classes/BcsType.md)\<`EnumOutputShape`\<\{ \[K in string \| number \| symbol\]: T\[K\] extends BcsType\<U, any\> ? U : true \}\>, `EnumInputShape`\<\{ \[K\_1 in string \| number \| symbol\]: T\[K\_1\] extends BcsType\<any, U\_1\> ? U\_1 : null \| boolean \| object \}\>\>

#### Example

```ts
const enum = bcs.enum('MyEnum', {
  A: bcs.u8(),
  B: bcs.string(),
  C: null,
})
enum.serialize({ A: 1 }).toBytes() // Uint8Array [ 0, 1 ]
enum.serialize({ B: 'a' }).toBytes() // Uint8Array [ 1, 1, 97 ]
enum.serialize({ C: true }).toBytes() // Uint8Array [ 2 ]
```

### map()

Creates a BcsType representing a map of a given key and value type

#### Type Parameters

• **K**

• **V**

• **InputK** = `K`

• **InputV** = `V`

#### Parameters

• **keyType**: [`BcsType`](../classes/BcsType.md)\<`K`, `InputK`\>

The BcsType of the key

• **valueType**: [`BcsType`](../classes/BcsType.md)\<`V`, `InputV`\>

The BcsType of the value

#### Returns

[`BcsType`](../classes/BcsType.md)\<`Map`\<`K`, `V`\>, `Map`\<`InputK`, `InputV`\>\>

#### Example

```ts
const map = bcs.map(bcs.u8(), bcs.string())
map.serialize(new Map([[2, 'a']])).toBytes() // Uint8Array [ 1, 2, 1, 97 ]
```

### lazy()

Creates a BcsType that wraps another BcsType which is lazily evaluated. This is useful for creating recursive types.

#### Type Parameters

• **T** _extends_ [`BcsType`](../classes/BcsType.md)\<`any`, `any`\>

#### Parameters

• **cb**

A callback that returns the BcsType

#### Returns

`T`

### U8

> **U8**: [`BcsType`](../classes/BcsType.md)\<`number`, `number`\>

### U16

> **U16**: [`BcsType`](../classes/BcsType.md)\<`number`, `number`\>

### U32

> **U32**: [`BcsType`](../classes/BcsType.md)\<`number`, `number`\>

### U64

> **U64**: [`BcsType`](../classes/BcsType.md)\<`string`, `string` \| `number` \| `bigint`\>

### U128

> **U128**: [`BcsType`](../classes/BcsType.md)\<`string`, `string` \| `number` \| `bigint`\>

### U256

> **U256**: [`BcsType`](../classes/BcsType.md)\<`string`, `string` \| `number` \| `bigint`\>

### ULEB128

> **ULEB128**: [`BcsType`](../classes/BcsType.md)\<`number`, `number`\>

### Bool

> **Bool**: [`BcsType`](../classes/BcsType.md)\<`boolean`, `boolean`\>

### String

> **String**: [`BcsType`](../classes/BcsType.md)\<`string`, `string`\>

### Address

> **Address**: [`BcsType`](../classes/BcsType.md)\<`string`, `string` \| `Uint8Array`\>

### AppId

> **AppId**: [`BcsType`](../classes/BcsType.md)\<`object`, `object`\>

#### Type declaration

##### $kind

> **$kind**: `"Iota"`

##### Iota

> **Iota**: `true` = `null`

### Argument

> **Argument**: [`BcsType`](../classes/BcsType.md)\<`EnumOutputShapeWithKeys`\<`object`, `"GasCoin"` \| `"Input"` \| `"Result"` \| `"NestedResult"`\>, `EnumInputShape`\<`object`\>\>

### CallArg

> **CallArg**: [`BcsType`](../classes/BcsType.md)\<`EnumOutputShapeWithKeys`\<`object`, `"Pure"` \| `"Object"`\>, `EnumInputShape`\<`object`\>\>

### CompressedSignature

> **CompressedSignature**: [`BcsType`](../classes/BcsType.md)\<`EnumOutputShapeWithKeys`\<`object`, `"ED25519"` \| `"Secp256k1"` \| `"Secp256r1"`\>, `EnumInputShape`\<`object`\>\>

### GasData

> **GasData**: [`BcsType`](../classes/BcsType.md)\<`object`, `object`\>

#### Type declaration

##### payment

> **payment**: `object`[]

##### owner

> **owner**: `string` = `Address`

##### price

> **price**: `string`

##### budget

> **budget**: `string`

### Intent

> **Intent**: [`BcsType`](../classes/BcsType.md)\<`object`, `object`\>

#### Type declaration

##### scope

> **scope**: `EnumOutputShapeWithKeys`\<`object`, `"TransactionData"` \| `"TransactionEffects"` \| `"CheckpointSummary"` \| `"PersonalMessage"`\> = `IntentScope`

###### Type declaration

###### TransactionData

> **TransactionData**: `true` = `null`

###### TransactionEffects

> **TransactionEffects**: `true` = `null`

###### CheckpointSummary

> **CheckpointSummary**: `true` = `null`

###### PersonalMessage

> **PersonalMessage**: `true` = `null`

##### version

> **version**: `object` = `IntentVersion`

##### version.$kind

> **$kind**: `"V0"`

##### version.V0

> **V0**: `true` = `null`

##### appId

> **appId**: `object` = `AppId`

##### appId.$kind

> **$kind**: `"Iota"`

##### appId.Iota

> **Iota**: `true` = `null`

### IntentMessage()

> **IntentMessage**: \<`T`\>(`T`) => [`BcsType`](../classes/BcsType.md)\<`object`, `object`\>

#### Type Parameters

• **T** _extends_ [`BcsType`](../classes/BcsType.md)\<`any`, `any`\>

#### Parameters

• **T**: `T`

#### Returns

[`BcsType`](../classes/BcsType.md)\<`object`, `object`\>

##### intent

> **intent**: `object` = `Intent`

##### intent.scope

> **scope**: `EnumOutputShapeWithKeys`\<`object`, `"TransactionData"` \| `"TransactionEffects"` \| `"CheckpointSummary"` \| `"PersonalMessage"`\> = `IntentScope`

###### Type declaration

###### TransactionData

> **TransactionData**: `true` = `null`

###### TransactionEffects

> **TransactionEffects**: `true` = `null`

###### CheckpointSummary

> **CheckpointSummary**: `true` = `null`

###### PersonalMessage

> **PersonalMessage**: `true` = `null`

##### intent.version

> **version**: `object` = `IntentVersion`

##### intent.version.$kind

> **$kind**: `"V0"`

##### intent.version.V0

> **V0**: `true` = `null`

##### intent.appId

> **appId**: `object` = `AppId`

##### intent.appId.$kind

> **$kind**: `"Iota"`

##### intent.appId.Iota

> **Iota**: `true` = `null`

##### value

> **value**: `T` _extends_ [`BcsType`](../classes/BcsType.md)\<`U`, `any`\> ? `U` : `never` = `T`

### IntentScope

> **IntentScope**: [`BcsType`](../classes/BcsType.md)\<`EnumOutputShapeWithKeys`\<`object`, `"TransactionData"` \| `"TransactionEffects"` \| `"CheckpointSummary"` \| `"PersonalMessage"`\>, `EnumInputShape`\<`object`\>\>

### IntentVersion

> **IntentVersion**: [`BcsType`](../classes/BcsType.md)\<`object`, `object`\>

#### Type declaration

##### $kind

> **$kind**: `"V0"`

##### V0

> **V0**: `true` = `null`

### MultiSig

> **MultiSig**: [`BcsType`](../classes/BcsType.md)\<`object`, `object`\>

#### Type declaration

##### sigs

> **sigs**: `EnumOutputShapeWithKeys`\<`object`, `"ED25519"` \| `"Secp256k1"` \| `"Secp256r1"`\>[]

##### bitmap

> **bitmap**: `number`

##### multisig\_pk

> **multisig\_pk**: `object` = `MultiSigPublicKey`

##### multisig\_pk.pk\_map

> **pk\_map**: `object`[]

##### multisig\_pk.threshold

> **threshold**: `number`

### MultiSigPkMap

> **MultiSigPkMap**: [`BcsType`](../classes/BcsType.md)\<`object`, `object`\>

#### Type declaration

##### pubKey

> **pubKey**: `EnumOutputShapeWithKeys`\<`object`, `"ED25519"` \| `"Secp256k1"` \| `"Secp256r1"`\> = `PublicKey`

###### Type declaration

###### ED25519

> **ED25519**: `number`[]

###### Secp256k1

> **Secp256k1**: `number`[]

###### Secp256r1

> **Secp256r1**: `number`[]

##### weight

> **weight**: `number`

### MultiSigPublicKey

> **MultiSigPublicKey**: [`BcsType`](../classes/BcsType.md)\<`object`, `object`\>

#### Type declaration

##### pk\_map

> **pk\_map**: `object`[]

##### threshold

> **threshold**: `number`

### ObjectArg

> **ObjectArg**: [`BcsType`](../classes/BcsType.md)\<`EnumOutputShapeWithKeys`\<`object`, `"ImmOrOwnedObject"` \| `"SharedObject"` \| `"Receiving"`\>, `EnumInputShape`\<`object`\>\>

### ObjectDigest

> **ObjectDigest**: [`BcsType`](../classes/BcsType.md)\<`string`, `string`\>

### ProgrammableMoveCall

> **ProgrammableMoveCall**: [`BcsType`](../classes/BcsType.md)\<`object`, `object`\>

#### Type declaration

##### package

> **package**: `string` = `Address`

##### module

> **module**: `string`

##### function

> **function**: `string`

##### typeArguments

> **typeArguments**: `string`[]

##### arguments

> **arguments**: `EnumOutputShapeWithKeys`\<`object`, `"GasCoin"` \| `"Input"` \| `"Result"` \| `"NestedResult"`\>[]

### ProgrammableTransaction

> **ProgrammableTransaction**: [`BcsType`](../classes/BcsType.md)\<`object`, `object`\>

#### Type declaration

##### inputs

> **inputs**: `EnumOutputShapeWithKeys`\<`object`, `"Pure"` \| `"Object"`\>[]

##### commands

> **commands**: `EnumOutputShapeWithKeys`\<`object`, `"MoveCall"` \| `"TransferObjects"` \| `"SplitCoins"` \| `"MergeCoins"` \| `"Publish"` \| `"MakeMoveVec"` \| `"Upgrade"`\>[]

### PublicKey

> **PublicKey**: [`BcsType`](../classes/BcsType.md)\<`EnumOutputShapeWithKeys`\<`object`, `"ED25519"` \| `"Secp256k1"` \| `"Secp256r1"`\>, `EnumInputShape`\<`object`\>\>

### SenderSignedData

> **SenderSignedData**: [`BcsType`](../classes/BcsType.md)\<`object`[], `Iterable`\<`object`, `any`, `any`\> & `object`\>

### SenderSignedTransaction

> **SenderSignedTransaction**: [`BcsType`](../classes/BcsType.md)\<`object`, `object`\>

#### Type declaration

##### intentMessage

> **intentMessage**: `object`

##### intentMessage.intent

> **intent**: `object` = `Intent`

##### intentMessage.intent.scope

> **scope**: `EnumOutputShapeWithKeys`\<`object`, `"TransactionData"` \| `"TransactionEffects"` \| `"CheckpointSummary"` \| `"PersonalMessage"`\> = `IntentScope`

###### Type declaration

###### TransactionData

> **TransactionData**: `true` = `null`

###### TransactionEffects

> **TransactionEffects**: `true` = `null`

###### CheckpointSummary

> **CheckpointSummary**: `true` = `null`

###### PersonalMessage

> **PersonalMessage**: `true` = `null`

##### intentMessage.intent.version

> **version**: `object` = `IntentVersion`

##### intentMessage.intent.version.$kind

> **$kind**: `"V0"`

##### intentMessage.intent.version.V0

> **V0**: `true` = `null`

##### intentMessage.intent.appId

> **appId**: `object` = `AppId`

##### intentMessage.intent.appId.$kind

> **$kind**: `"Iota"`

##### intentMessage.intent.appId.Iota

> **Iota**: `true` = `null`

##### intentMessage.value

> **value**: `object` = `T`

##### intentMessage.value.$kind

> **$kind**: `"V1"`

##### intentMessage.value.V1

> **V1**: `object` = `TransactionDataV1`

##### intentMessage.value.V1.kind

> **kind**: `EnumOutputShapeWithKeys`\<`object`, `"ProgrammableTransaction"` \| `"ChangeEpoch"` \| `"Genesis"` \| `"ConsensusCommitPrologue"`\> = `TransactionKind`

###### Type declaration

###### ProgrammableTransaction

> **ProgrammableTransaction**: `object`

###### ProgrammableTransaction.inputs

> **inputs**: `EnumOutputShapeWithKeys`\<..., ...\>[]

###### ProgrammableTransaction.commands

> **commands**: `EnumOutputShapeWithKeys`\<..., ...\>[]

###### ChangeEpoch

> **ChangeEpoch**: `true` = `null`

###### Genesis

> **Genesis**: `true` = `null`

###### ConsensusCommitPrologue

> **ConsensusCommitPrologue**: `true` = `null`

##### intentMessage.value.V1.sender

> **sender**: `string` = `Address`

##### intentMessage.value.V1.gasData

> **gasData**: `object` = `GasData`

##### intentMessage.value.V1.gasData.payment

> **payment**: `object`[]

##### intentMessage.value.V1.gasData.owner

> **owner**: `string` = `Address`

##### intentMessage.value.V1.gasData.price

> **price**: `string`

##### intentMessage.value.V1.gasData.budget

> **budget**: `string`

##### intentMessage.value.V1.expiration

> **expiration**: `EnumOutputShapeWithKeys`\<`object`, `"None"` \| `"Epoch"`\> = `TransactionExpiration`

###### Type declaration

###### None

> **None**: `true` = `null`

###### Epoch

> **Epoch**: `number`

##### txSignatures

> **txSignatures**: `string`[]

### SharedObjectRef

> **SharedObjectRef**: [`BcsType`](../classes/BcsType.md)\<`object`, `object`\>

#### Type declaration

##### objectId

> **objectId**: `string` = `Address`

##### initialSharedVersion

> **initialSharedVersion**: `string`

##### mutable

> **mutable**: `boolean`

### StructTag

> **StructTag**: [`BcsType`](../classes/BcsType.md)\<`object`, `object`\>

#### Type declaration

##### address

> **address**: `string` = `Address`

##### module

> **module**: `string`

##### name

> **name**: `string`

##### typeParams

> **typeParams**: [`TypeTag`](../type-aliases/TypeTag.md)[]

### IotaObjectRef

> **IotaObjectRef**: [`BcsType`](../classes/BcsType.md)\<`object`, `object`\>

#### Type declaration

##### objectId

> **objectId**: `string` = `Address`

##### version

> **version**: `string`

##### digest

> **digest**: `string` = `ObjectDigest`

### Command

> **Command**: [`BcsType`](../classes/BcsType.md)\<`EnumOutputShapeWithKeys`\<`object`, `"MoveCall"` \| `"TransferObjects"` \| `"SplitCoins"` \| `"MergeCoins"` \| `"Publish"` \| `"MakeMoveVec"` \| `"Upgrade"`\>, `EnumInputShape`\<`object`\>\>

### TransactionData

> **TransactionData**: [`BcsType`](../classes/BcsType.md)\<`object`, `object`\>

#### Type declaration

##### $kind

> **$kind**: `"V1"`

##### V1

> **V1**: `object` = `TransactionDataV1`

##### V1.kind

> **kind**: `EnumOutputShapeWithKeys`\<`object`, `"ProgrammableTransaction"` \| `"ChangeEpoch"` \| `"Genesis"` \| `"ConsensusCommitPrologue"`\> = `TransactionKind`

###### Type declaration

###### ProgrammableTransaction

> **ProgrammableTransaction**: `object`

###### ProgrammableTransaction.inputs

> **inputs**: `EnumOutputShapeWithKeys`\<`object`, `"Pure"` \| `"Object"`\>[]

###### ProgrammableTransaction.commands

> **commands**: `EnumOutputShapeWithKeys`\<`object`, `"MoveCall"` \| `"TransferObjects"` \| `"SplitCoins"` \| `"MergeCoins"` \| `"Publish"` \| `"MakeMoveVec"` \| `"Upgrade"`\>[]

###### ChangeEpoch

> **ChangeEpoch**: `true` = `null`

###### Genesis

> **Genesis**: `true` = `null`

###### ConsensusCommitPrologue

> **ConsensusCommitPrologue**: `true` = `null`

##### V1.sender

> **sender**: `string` = `Address`

##### V1.gasData

> **gasData**: `object` = `GasData`

##### V1.gasData.payment

> **payment**: `object`[]

##### V1.gasData.owner

> **owner**: `string` = `Address`

##### V1.gasData.price

> **price**: `string`

##### V1.gasData.budget

> **budget**: `string`

##### V1.expiration

> **expiration**: `EnumOutputShapeWithKeys`\<`object`, `"None"` \| `"Epoch"`\> = `TransactionExpiration`

###### Type declaration

###### None

> **None**: `true` = `null`

###### Epoch

> **Epoch**: `number`

### TransactionDataV1

> **TransactionDataV1**: [`BcsType`](../classes/BcsType.md)\<`object`, `object`\>

#### Type declaration

##### kind

> **kind**: `EnumOutputShapeWithKeys`\<`object`, `"ProgrammableTransaction"` \| `"ChangeEpoch"` \| `"Genesis"` \| `"ConsensusCommitPrologue"`\> = `TransactionKind`

###### Type declaration

###### ProgrammableTransaction

> **ProgrammableTransaction**: `object`

###### ProgrammableTransaction.inputs

> **inputs**: `EnumOutputShapeWithKeys`\<`object`, `"Pure"` \| `"Object"`\>[]

###### ProgrammableTransaction.commands

> **commands**: `EnumOutputShapeWithKeys`\<`object`, `"MoveCall"` \| `"TransferObjects"` \| `"SplitCoins"` \| `"MergeCoins"` \| `"Publish"` \| `"MakeMoveVec"` \| `"Upgrade"`\>[]

###### ChangeEpoch

> **ChangeEpoch**: `true` = `null`

###### Genesis

> **Genesis**: `true` = `null`

###### ConsensusCommitPrologue

> **ConsensusCommitPrologue**: `true` = `null`

##### sender

> **sender**: `string` = `Address`

##### gasData

> **gasData**: `object` = `GasData`

##### gasData.payment

> **payment**: `object`[]

##### gasData.owner

> **owner**: `string` = `Address`

##### gasData.price

> **price**: `string`

##### gasData.budget

> **budget**: `string`

##### expiration

> **expiration**: `EnumOutputShapeWithKeys`\<`object`, `"None"` \| `"Epoch"`\> = `TransactionExpiration`

###### Type declaration

###### None

> **None**: `true` = `null`

###### Epoch

> **Epoch**: `number`

### TransactionExpiration

> **TransactionExpiration**: [`BcsType`](../classes/BcsType.md)\<`EnumOutputShapeWithKeys`\<`object`, `"None"` \| `"Epoch"`\>, `EnumInputShape`\<`object`\>\>

### TransactionKind

> **TransactionKind**: [`BcsType`](../classes/BcsType.md)\<`EnumOutputShapeWithKeys`\<`object`, `"ProgrammableTransaction"` \| `"ChangeEpoch"` \| `"Genesis"` \| `"ConsensusCommitPrologue"`\>, `EnumInputShape`\<`object`\>\>

### TypeTag

> **TypeTag**: [`BcsType`](../classes/BcsType.md)\<`string`, `string` \| [`TypeTag`](../type-aliases/TypeTag.md)\>

### TransactionEffects

> **TransactionEffects**: [`BcsType`](../classes/BcsType.md)\<`object`, `object`\>

#### Type declaration

##### $kind

> **$kind**: `"V1"`

##### V1

> **V1**: `object` = `TransactionEffectsV1`

##### V1.status

> **status**: `EnumOutputShapeWithKeys`\<`object`, `"Success"` \| `"Failed"`\> = `ExecutionStatus`

###### Type declaration

###### Success

> **Success**: `true` = `null`

###### Failed

> **Failed**: `object`

###### Failed.error

> **error**: `EnumOutputShapeWithKeys`\<`object`, `"PackageUpgradeError"` \| `"SharedObjectOperationNotAllowed"` \| `"CommandArgumentError"` \| `"TypeArgumentError"` \| `"InsufficientGas"` \| `"InvalidGasObject"` \| `"InvariantViolation"` \| `"FeatureNotYetSupported"` \| `"MoveObjectTooBig"` \| `"MovePackageTooBig"` \| `"CircularObjectOwnership"` \| `"InsufficientCoinBalance"` \| `"CoinBalanceOverflow"` \| `"PublishErrorNonZeroAddress"` \| `"IotaMoveVerificationError"` \| `"MovePrimitiveRuntimeError"` \| `"MoveAbort"` \| `"VMVerificationOrDeserializationError"` \| `"VMInvariantViolation"` \| `"FunctionNotFound"` \| `"ArityMismatch"` \| `"TypeArityMismatch"` \| `"NonEntryFunctionInvoked"` \| `"UnusedValueWithoutDrop"` \| `"InvalidPublicFunctionReturnType"` \| `"InvalidTransferObject"` \| `"EffectsTooLarge"` \| `"PublishUpgradeMissingDependency"` \| `"PublishUpgradeDependencyDowngrade"` \| `"WrittenObjectsTooLarge"` \| `"CertificateDenied"` \| `"IotaMoveVerificationTimedout"` \| `"InputObjectDeleted"`\> = `ExecutionFailureStatus`

###### Type declaration

###### InsufficientGas

> **InsufficientGas**: `true` = `null`

###### InvalidGasObject

> **InvalidGasObject**: `true` = `null`

###### InvariantViolation

> **InvariantViolation**: `true` = `null`

###### FeatureNotYetSupported

> **FeatureNotYetSupported**: `true` = `null`

###### MoveObjectTooBig

> **MoveObjectTooBig**: `object`

###### MoveObjectTooBig.objectSize

> **objectSize**: `string`

###### MoveObjectTooBig.maxObjectSize

> **maxObjectSize**: `string`

###### MovePackageTooBig

> **MovePackageTooBig**: `object`

###### MovePackageTooBig.objectSize

> **objectSize**: `string`

###### MovePackageTooBig.maxObjectSize

> **maxObjectSize**: `string`

###### CircularObjectOwnership

> **CircularObjectOwnership**: `object`

###### CircularObjectOwnership.object

> **object**: `string` = `Address`

###### InsufficientCoinBalance

> **InsufficientCoinBalance**: `true` = `null`

###### CoinBalanceOverflow

> **CoinBalanceOverflow**: `true` = `null`

###### PublishErrorNonZeroAddress

> **PublishErrorNonZeroAddress**: `true` = `null`

###### IotaMoveVerificationError

> **IotaMoveVerificationError**: `true` = `null`

###### MovePrimitiveRuntimeError

> **MovePrimitiveRuntimeError**: `null` \| `object`

###### MoveAbort

> **MoveAbort**: [`object`, `string`]

###### VMVerificationOrDeserializationError

> **VMVerificationOrDeserializationError**: `true` = `null`

###### VMInvariantViolation

> **VMInvariantViolation**: `true` = `null`

###### FunctionNotFound

> **FunctionNotFound**: `true` = `null`

###### ArityMismatch

> **ArityMismatch**: `true` = `null`

###### TypeArityMismatch

> **TypeArityMismatch**: `true` = `null`

###### NonEntryFunctionInvoked

> **NonEntryFunctionInvoked**: `true` = `null`

###### CommandArgumentError

> **CommandArgumentError**: `object`

###### CommandArgumentError.argIdx

> **argIdx**: `number`

###### CommandArgumentError.kind

> **kind**: `EnumOutputShapeWithKeys`\<..., ...\> = `CommandArgumentError`

###### TypeArgumentError

> **TypeArgumentError**: `object`

###### TypeArgumentError.argumentIdx

> **argumentIdx**: `number`

###### TypeArgumentError.kind

> **kind**: `EnumOutputShapeWithKeys`\<..., ...\> = `TypeArgumentError`

###### UnusedValueWithoutDrop

> **UnusedValueWithoutDrop**: `object`

###### UnusedValueWithoutDrop.resultIdx

> **resultIdx**: `number`

###### UnusedValueWithoutDrop.secondaryIdx

> **secondaryIdx**: `number`

###### InvalidPublicFunctionReturnType

> **InvalidPublicFunctionReturnType**: `object`

###### InvalidPublicFunctionReturnType.idx

> **idx**: `number`

###### InvalidTransferObject

> **InvalidTransferObject**: `true` = `null`

###### EffectsTooLarge

> **EffectsTooLarge**: `object`

###### EffectsTooLarge.currentSize

> **currentSize**: `string`

###### EffectsTooLarge.maxSize

> **maxSize**: `string`

###### PublishUpgradeMissingDependency

> **PublishUpgradeMissingDependency**: `true` = `null`

###### PublishUpgradeDependencyDowngrade

> **PublishUpgradeDependencyDowngrade**: `true` = `null`

###### PackageUpgradeError

> **PackageUpgradeError**: `object`

###### PackageUpgradeError.upgradeError

> **upgradeError**: `EnumOutputShapeWithKeys`\<..., ...\> = `PackageUpgradeError`

###### WrittenObjectsTooLarge

> **WrittenObjectsTooLarge**: `object`

###### WrittenObjectsTooLarge.currentSize

> **currentSize**: `string`

###### WrittenObjectsTooLarge.maxSize

> **maxSize**: `string`

###### CertificateDenied

> **CertificateDenied**: `true` = `null`

###### IotaMoveVerificationTimedout

> **IotaMoveVerificationTimedout**: `true` = `null`

###### SharedObjectOperationNotAllowed

> **SharedObjectOperationNotAllowed**: `true` = `null`

###### InputObjectDeleted

> **InputObjectDeleted**: `true` = `null`

###### Failed.command

> **command**: `null` \| `string`

##### V1.executedEpoch

> **executedEpoch**: `string`

##### V1.gasUsed

> **gasUsed**: `object` = `GasCostSummary`

##### V1.gasUsed.computationCost

> **computationCost**: `string`

##### V1.gasUsed.computationCostBurned

> **computationCostBurned**: `string`

##### V1.gasUsed.storageCost

> **storageCost**: `string`

##### V1.gasUsed.storageRebate

> **storageRebate**: `string`

##### V1.gasUsed.nonRefundableStorageFee

> **nonRefundableStorageFee**: `string`

##### V1.transactionDigest

> **transactionDigest**: `string` = `ObjectDigest`

##### V1.gasObjectIndex

> **gasObjectIndex**: `null` \| `number`

##### V1.eventsDigest

> **eventsDigest**: `null` \| `string`

##### V1.dependencies

> **dependencies**: `string`[]

##### V1.lamportVersion

> **lamportVersion**: `string`

##### V1.changedObjects

> **changedObjects**: [`string`, `object`][]

##### V1.unchangedSharedObjects

> **unchangedSharedObjects**: [`string`, `EnumOutputShapeWithKeys`\<`object`, `"ReadOnlyRoot"` \| `"MutateDeleted"` \| `"ReadDeleted"` \| `"Cancelled"` \| `"PerEpochConfig"`\>][]

##### V1.auxDataDigest

> **auxDataDigest**: `null` \| `string`
