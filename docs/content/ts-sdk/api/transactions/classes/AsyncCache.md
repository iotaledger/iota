# Class: `abstract` AsyncCache

## Constructors

### new AsyncCache()

> **new AsyncCache**(): [`AsyncCache`](AsyncCache.md)

#### Returns

[`AsyncCache`](AsyncCache.md)

## Methods

### get()

> `abstract` `protected` **get**\<`T`\>(`type`, `key`): `Promise`\<`null` \| `CacheEntryTypes`\[`T`\]\>

#### Type Parameters

• **T** _extends_ keyof `CacheEntryTypes`

#### Parameters

• **type**: `T`

• **key**: `string`

#### Returns

`Promise`\<`null` \| `CacheEntryTypes`\[`T`\]\>

---

### set()

> `abstract` `protected` **set**\<`T`\>(`type`, `key`, `value`): `Promise`\<`void`\>

#### Type Parameters

• **T** _extends_ keyof `CacheEntryTypes`

#### Parameters

• **type**: `T`

• **key**: `string`

• **value**: `CacheEntryTypes`\[`T`\]

#### Returns

`Promise`\<`void`\>

---

### delete()

> `abstract` `protected` **delete**\<`T`\>(`type`, `key`): `Promise`\<`void`\>

#### Type Parameters

• **T** _extends_ keyof `CacheEntryTypes`

#### Parameters

• **type**: `T`

• **key**: `string`

#### Returns

`Promise`\<`void`\>

---

### clear()

> `abstract` **clear**\<`T`\>(`type`?): `Promise`\<`void`\>

#### Type Parameters

• **T** _extends_ keyof `CacheEntryTypes`

#### Parameters

• **type?**: `T`

#### Returns

`Promise`\<`void`\>

---

### getObject()

> **getObject**(`id`): `Promise`\<`null` \| `ObjectCacheEntry`\>

#### Parameters

• **id**: `string`

#### Returns

`Promise`\<`null` \| `ObjectCacheEntry`\>

---

### getObjects()

> **getObjects**(`ids`): `Promise`\<(`null` \| `ObjectCacheEntry`)[]\>

#### Parameters

• **ids**: `string`[]

#### Returns

`Promise`\<(`null` \| `ObjectCacheEntry`)[]\>

---

### addObject()

> **addObject**(`object`): `Promise`\<`ObjectCacheEntry`\>

#### Parameters

• **object**: `ObjectCacheEntry`

#### Returns

`Promise`\<`ObjectCacheEntry`\>

---

### addObjects()

> **addObjects**(`objects`): `Promise`\<`void`\>

#### Parameters

• **objects**: `ObjectCacheEntry`[]

#### Returns

`Promise`\<`void`\>

---

### deleteObject()

> **deleteObject**(`id`): `Promise`\<`void`\>

#### Parameters

• **id**: `string`

#### Returns

`Promise`\<`void`\>

---

### deleteObjects()

> **deleteObjects**(`ids`): `Promise`\<`void`\>

#### Parameters

• **ids**: `string`[]

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

### addMoveFunctionDefinition()

> **addMoveFunctionDefinition**(`functionEntry`): `Promise`\<`object`\>

#### Parameters

• **functionEntry**: `MoveFunctionCacheEntry`

#### Returns

`Promise`\<`object`\>

##### module

> **module**: `string`

##### function

> **function**: `string`

##### parameters

> **parameters**: `object`[]

##### package

> **package**: `string` = `pkg`

---

### deleteMoveFunctionDefinition()

> **deleteMoveFunctionDefinition**(`ref`): `Promise`\<`void`\>

#### Parameters

• **ref**

• **ref.package**: `string`

• **ref.module**: `string`

• **ref.function**: `string`

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
