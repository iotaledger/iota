# Interface: IotaObjectData

## Properties

### bcs?

> `optional` **bcs**: `null` \| [`RawData`](../type-aliases/RawData.md)

Move object content or package content in BCS, default to be None unless
IotaObjectDataOptions.showBcs is set to true

---

### content?

> `optional` **content**: `null` \| [`IotaParsedData`](../type-aliases/IotaParsedData.md)

Move object content or package content, default to be None unless IotaObjectDataOptions.showContent
is set to true

---

### digest

> **digest**: `string`

Base64 string representing the object digest

---

### display?

> `optional` **display**: `null` \| [`DisplayFieldsResponse`](DisplayFieldsResponse.md)

The Display metadata for frontend UI rendering, default to be None unless
IotaObjectDataOptions.showContent is set to true This can also be None if the struct type does not
have Display defined

---

### objectId

> **objectId**: `string`

---

### owner?

> `optional` **owner**: `null` \| [`ObjectOwner`](../type-aliases/ObjectOwner.md)

The owner of this object. Default to be None unless IotaObjectDataOptions.showOwner is set to true

---

### previousTransaction?

> `optional` **previousTransaction**: `null` \| `string`

The digest of the transaction that created or last mutated this object. Default to be None unless
IotaObjectDataOptions. showPreviousTransaction is set to true

---

### storageRebate?

> `optional` **storageRebate**: `null` \| `string`

The amount of IOTA we would rebate if this object gets deleted. This number is re-calculated each
time the object is mutated based on the present storage gas price.

---

### type?

> `optional` **type**: `null` \| `string`

The type of the object. Default to be None unless IotaObjectDataOptions.showType is set to true

---

### version

> **version**: `string`

Object version.
