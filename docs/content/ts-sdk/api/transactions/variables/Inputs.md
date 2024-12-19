# Variable: Inputs

> `const` **Inputs**: `object`

## Type declaration

### Pure()

> **Pure**: (`data`) => `Extract`\<[`CallArg`](../type-aliases/CallArg.md), `object`\>

#### Parameters

• **data**: `Uint8Array` \| `SerializedBcs`\<`any`, `any`\>

#### Returns

`Extract`\<[`CallArg`](../type-aliases/CallArg.md), `object`\>

### ObjectRef()

#### Parameters

• **\_\_namedParameters**

• **\_\_namedParameters.objectId**: `string`

• **\_\_namedParameters.version**: `string` \| `number`

• **\_\_namedParameters.digest**: `string`

#### Returns

`object`

##### Pure

> **Pure**: `undefined`

##### UnresolvedPure

> **UnresolvedPure**: `undefined`

##### UnresolvedObject

> **UnresolvedObject**: `undefined`

##### $kind

> **$kind**: `"Object"`

##### Object

> **Object**: `EnumOutputShapeWithKeys`\<`object`, `"ImmOrOwnedObject"` \| `"SharedObject"` \| `"Receiving"`\> = `ObjectArg`

###### Type declaration

###### ImmOrOwnedObject

> **ImmOrOwnedObject**: `object` = `ObjectRef`

###### ImmOrOwnedObject.objectId

> **objectId**: `string` = `IotaAddress`

###### ImmOrOwnedObject.version

> **version**: `string` \| `number` = `JsonU64`

###### ImmOrOwnedObject.digest

> **digest**: `string`

###### SharedObject

> **SharedObject**: `object`

###### SharedObject.objectId

> **objectId**: `string` = `ObjectID`

###### SharedObject.initialSharedVersion

> **initialSharedVersion**: `string` \| `number` = `JsonU64`

###### SharedObject.mutable

> **mutable**: `boolean`

###### Receiving

> **Receiving**: `object` = `ObjectRef`

###### Receiving.objectId

> **objectId**: `string` = `IotaAddress`

###### Receiving.version

> **version**: `string` \| `number` = `JsonU64`

###### Receiving.digest

> **digest**: `string`

### SharedObjectRef()

#### Parameters

• **\_\_namedParameters**

• **\_\_namedParameters.objectId**: `string`

• **\_\_namedParameters.mutable**: `boolean`

• **\_\_namedParameters.initialSharedVersion**: `string` \| `number`

#### Returns

`object`

##### Pure

> **Pure**: `undefined`

##### UnresolvedPure

> **UnresolvedPure**: `undefined`

##### UnresolvedObject

> **UnresolvedObject**: `undefined`

##### $kind

> **$kind**: `"Object"`

##### Object

> **Object**: `EnumOutputShapeWithKeys`\<`object`, `"ImmOrOwnedObject"` \| `"SharedObject"` \| `"Receiving"`\> = `ObjectArg`

###### Type declaration

###### ImmOrOwnedObject

> **ImmOrOwnedObject**: `object` = `ObjectRef`

###### ImmOrOwnedObject.objectId

> **objectId**: `string` = `IotaAddress`

###### ImmOrOwnedObject.version

> **version**: `string` \| `number` = `JsonU64`

###### ImmOrOwnedObject.digest

> **digest**: `string`

###### SharedObject

> **SharedObject**: `object`

###### SharedObject.objectId

> **objectId**: `string` = `ObjectID`

###### SharedObject.initialSharedVersion

> **initialSharedVersion**: `string` \| `number` = `JsonU64`

###### SharedObject.mutable

> **mutable**: `boolean`

###### Receiving

> **Receiving**: `object` = `ObjectRef`

###### Receiving.objectId

> **objectId**: `string` = `IotaAddress`

###### Receiving.version

> **version**: `string` \| `number` = `JsonU64`

###### Receiving.digest

> **digest**: `string`

### ReceivingRef()

#### Parameters

• **\_\_namedParameters**

• **\_\_namedParameters.objectId**: `string`

• **\_\_namedParameters.version**: `string` \| `number`

• **\_\_namedParameters.digest**: `string`

#### Returns

`object`

##### Pure

> **Pure**: `undefined`

##### UnresolvedPure

> **UnresolvedPure**: `undefined`

##### UnresolvedObject

> **UnresolvedObject**: `undefined`

##### $kind

> **$kind**: `"Object"`

##### Object

> **Object**: `EnumOutputShapeWithKeys`\<`object`, `"ImmOrOwnedObject"` \| `"SharedObject"` \| `"Receiving"`\> = `ObjectArg`

###### Type declaration

###### ImmOrOwnedObject

> **ImmOrOwnedObject**: `object` = `ObjectRef`

###### ImmOrOwnedObject.objectId

> **objectId**: `string` = `IotaAddress`

###### ImmOrOwnedObject.version

> **version**: `string` \| `number` = `JsonU64`

###### ImmOrOwnedObject.digest

> **digest**: `string`

###### SharedObject

> **SharedObject**: `object`

###### SharedObject.objectId

> **objectId**: `string` = `ObjectID`

###### SharedObject.initialSharedVersion

> **initialSharedVersion**: `string` \| `number` = `JsonU64`

###### SharedObject.mutable

> **mutable**: `boolean`

###### Receiving

> **Receiving**: `object` = `ObjectRef`

###### Receiving.objectId

> **objectId**: `string` = `IotaAddress`

###### Receiving.version

> **version**: `string` \| `number` = `JsonU64`

###### Receiving.digest

> **digest**: `string`
