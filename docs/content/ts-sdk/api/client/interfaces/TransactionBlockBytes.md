# Interface: TransactionBlockBytes

## Properties

### gas

> **gas**: [`IotaObjectRef`](IotaObjectRef.md)[]

the gas objects to be used

---

### inputObjects

> **inputObjects**: [`InputObjectKind`](../type-aliases/InputObjectKind.md)[]

objects to be used in this transaction

---

### txBytes

> **txBytes**: `string`

BCS serialized transaction data bytes without its type tag, as base-64 encoded string.
