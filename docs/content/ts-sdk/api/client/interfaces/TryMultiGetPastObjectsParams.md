# Interface: TryMultiGetPastObjectsParams

Note there is no software-level guarantee/SLA that objects with past versions can be retrieved by
this API, even if the object and version exists/existed. The result may vary across nodes depending
on their pruning policies. Return the object information for a specified version

## Properties

### pastObjects

> **pastObjects**: [`GetPastObjectRequest`](GetPastObjectRequest.md)[]

a vector of object and versions to be queried

---

### options?

> `optional` **options**: `null` \| [`IotaObjectDataOptions`](IotaObjectDataOptions.md)

options for specifying the content to be returned
