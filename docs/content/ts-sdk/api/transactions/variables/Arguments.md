# Variable: Arguments

> `const` **Arguments**: `object`

## Type declaration

### pure()

> **pure**: \<`T`\>(`type`, `value`) => `T`(`value`) => `T`

#### Type Parameters

• **T** _extends_ `PureTypeName`

#### Parameters

• **type**: `T` _extends_ `PureTypeName` ? `ValidPureTypeName`\<`T`\<`T`\>\> : `T`

• **value**: `ShapeFromPureTypeName`\<`T`\>

#### Returns

`T`

#### Parameters

• **value**: `Uint8Array` \| `SerializedBcs`\<`any`, `any`\>

The pure value, serialized to BCS. If this is a Uint8Array, then the value
is assumed to be raw bytes, and will be used directly.

#### Returns

`T`

### pure.u8()

#### Parameters

• **value**: `number`

#### Returns

`Function`

##### Parameters

• **tx**: [`Transaction`](../classes/Transaction.md)

##### Returns

`object` \| `object` \| `object` \| `object` \| `object`

### pure.u16()

#### Parameters

• **value**: `number`

#### Returns

`Function`

##### Parameters

• **tx**: [`Transaction`](../classes/Transaction.md)

##### Returns

`object` \| `object` \| `object` \| `object` \| `object`

### pure.u32()

#### Parameters

• **value**: `number`

#### Returns

`Function`

##### Parameters

• **tx**: [`Transaction`](../classes/Transaction.md)

##### Returns

`object` \| `object` \| `object` \| `object` \| `object`

### pure.u64()

#### Parameters

• **value**: `string` \| `number` \| `bigint`

#### Returns

`Function`

##### Parameters

• **tx**: [`Transaction`](../classes/Transaction.md)

##### Returns

`object` \| `object` \| `object` \| `object` \| `object`

### pure.u128()

#### Parameters

• **value**: `string` \| `number` \| `bigint`

#### Returns

`Function`

##### Parameters

• **tx**: [`Transaction`](../classes/Transaction.md)

##### Returns

`object` \| `object` \| `object` \| `object` \| `object`

### pure.u256()

#### Parameters

• **value**: `string` \| `number` \| `bigint`

#### Returns

`Function`

##### Parameters

• **tx**: [`Transaction`](../classes/Transaction.md)

##### Returns

`object` \| `object` \| `object` \| `object` \| `object`

### pure.bool()

#### Parameters

• **value**: `boolean`

#### Returns

`Function`

##### Parameters

• **tx**: [`Transaction`](../classes/Transaction.md)

##### Returns

`object` \| `object` \| `object` \| `object` \| `object`

### pure.string()

#### Parameters

• **value**: `string`

#### Returns

`Function`

##### Parameters

• **tx**: [`Transaction`](../classes/Transaction.md)

##### Returns

`object` \| `object` \| `object` \| `object` \| `object`

### pure.address()

#### Parameters

• **value**: `string`

#### Returns

`Function`

##### Parameters

• **tx**: [`Transaction`](../classes/Transaction.md)

##### Returns

`object` \| `object` \| `object` \| `object` \| `object`

### pure.id()

> **id**: (`value`) => (`tx`) => `object` \| `object` \| `object` \| `object` \| `object`

#### Parameters

• **value**: `string`

#### Returns

`Function`

##### Parameters

• **tx**: [`Transaction`](../classes/Transaction.md)

##### Returns

`object` \| `object` \| `object` \| `object` \| `object`

### pure.vector()

#### Type Parameters

• **Type** _extends_ `PureTypeName`

#### Parameters

• **type**: `Type`

• **value**: `Iterable`\<`ShapeFromPureTypeName`\<`Type`\>, `any`, `any`\> & `object`

#### Returns

`Function`

##### Parameters

• **tx**: [`Transaction`](../classes/Transaction.md)

##### Returns

`object` \| `object` \| `object` \| `object` \| `object`

### pure.option()

#### Type Parameters

• **Type** _extends_ `PureTypeName`

#### Parameters

• **type**: `Type`

• **value**: `undefined` \| `null` \| `ShapeFromPureTypeName`\<`Type`\>

#### Returns

`Function`

##### Parameters

• **tx**: [`Transaction`](../classes/Transaction.md)

##### Returns

`object` \| `object` \| `object` \| `object` \| `object`

### object()

> **object**: (`value`) => (`tx`) => `object`

#### Parameters

• **value**: [`TransactionObjectInput`](../type-aliases/TransactionObjectInput.md)

#### Returns

`Function`

##### Parameters

• **tx**: [`Transaction`](../classes/Transaction.md)

##### Returns

`object`

###### $kind

> **$kind**: `"Input"`

###### Input

> **Input**: `number`

###### type?

> `optional` **type**: `"object"`

### object.system()

#### Returns

`Function`

##### Parameters

• **tx**: [`Transaction`](../classes/Transaction.md)

##### Returns

`object`

###### $kind

> **$kind**: `"Input"`

###### Input

> **Input**: `number`

###### type?

> `optional` **type**: `"object"`

### object.clock()

#### Returns

`Function`

##### Parameters

• **tx**: [`Transaction`](../classes/Transaction.md)

##### Returns

`object`

###### $kind

> **$kind**: `"Input"`

###### Input

> **Input**: `number`

###### type?

> `optional` **type**: `"object"`

### object.random()

#### Returns

`Function`

##### Parameters

• **tx**: [`Transaction`](../classes/Transaction.md)

##### Returns

`object`

###### $kind

> **$kind**: `"Input"`

###### Input

> **Input**: `number`

###### type?

> `optional` **type**: `"object"`

### object.denyList()

#### Returns

`Function`

##### Parameters

• **tx**: [`Transaction`](../classes/Transaction.md)

##### Returns

`object`

###### $kind

> **$kind**: `"Input"`

###### Input

> **Input**: `number`

###### type?

> `optional` **type**: `"object"`

### sharedObjectRef()

> **sharedObjectRef**: (...`args`) => (`tx`) => `object`

#### Parameters

• ...**args**: [`object`]

#### Returns

`Function`

##### Parameters

• **tx**: [`Transaction`](../classes/Transaction.md)

##### Returns

`object`

###### $kind

> **$kind**: `"Input"`

###### Input

> **Input**: `number`

###### type?

> `optional` **type**: `"object"`

### objectRef()

> **objectRef**: (...`args`) => (`tx`) => `object`

#### Parameters

• ...**args**: [`object`]

#### Returns

`Function`

##### Parameters

• **tx**: [`Transaction`](../classes/Transaction.md)

##### Returns

`object`

###### $kind

> **$kind**: `"Input"`

###### Input

> **Input**: `number`

###### type?

> `optional` **type**: `"object"`

### receivingRef()

> **receivingRef**: (...`args`) => (`tx`) => `object`

#### Parameters

• ...**args**: [`object`]

#### Returns

`Function`

##### Parameters

• **tx**: [`Transaction`](../classes/Transaction.md)

##### Returns

`object`

###### $kind

> **$kind**: `"Input"`

###### Input

> **Input**: `number`

###### type?

> `optional` **type**: `"object"`
