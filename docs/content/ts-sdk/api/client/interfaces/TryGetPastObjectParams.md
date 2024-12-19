# Interface: TryGetPastObjectParams

Note there is no software-level guarantee/SLA that objects with past versions can be retrieved by
this API, even if the object and version exists/existed. The result may vary across nodes depending
on their pruning policies. Return the object information for a specified version

## Properties

### id

> **id**: `string`

the ID of the queried object

---

### version

> **version**: `number`

the version of the queried object. If None, default to the latest known version

---

### options?

> `optional` **options**: `null` \| [`IotaObjectDataOptions`](IotaObjectDataOptions.md)

options for specifying the content to be returned
