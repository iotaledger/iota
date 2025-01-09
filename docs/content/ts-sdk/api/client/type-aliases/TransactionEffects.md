# Type Alias: TransactionEffects

> **TransactionEffects**: `object`

## Type declaration

### created?

> `optional` **created**: [`OwnedObjectRef`](../interfaces/OwnedObjectRef.md)[]

ObjectRef and owner of new objects created.

### deleted?

> `optional` **deleted**: [`IotaObjectRef`](../interfaces/IotaObjectRef.md)[]

Object Refs of objects now deleted (the old refs).

### dependencies?

> `optional` **dependencies**: `string`[]

The set of transaction digests this transaction depends on.

### eventsDigest?

> `optional` **eventsDigest**: `string` \| `null`

The digest of the events emitted during execution, can be None if the transaction does not emit any
event.

### executedEpoch

> **executedEpoch**: `string`

The epoch when this transaction was executed.

### gasObject

> **gasObject**: [`OwnedObjectRef`](../interfaces/OwnedObjectRef.md)

The updated gas object reference. Have a dedicated field for convenient access. It's also included
in mutated.

### gasUsed

> **gasUsed**: [`GasCostSummary`](../interfaces/GasCostSummary.md)

### messageVersion

> **messageVersion**: `"v1"`

### modifiedAtVersions?

> `optional` **modifiedAtVersions**: [`TransactionBlockEffectsModifiedAtVersions`](../interfaces/TransactionBlockEffectsModifiedAtVersions.md)[]

The version that every modified (mutated or deleted) object had before it was modified by this
transaction.

### mutated?

> `optional` **mutated**: [`OwnedObjectRef`](../interfaces/OwnedObjectRef.md)[]

ObjectRef and owner of mutated objects, including gas object.

### sharedObjects?

> `optional` **sharedObjects**: [`IotaObjectRef`](../interfaces/IotaObjectRef.md)[]

The object references of the shared objects used in this transaction. Empty if no shared objects
were used.

### status

> **status**: [`ExecutionStatus`](ExecutionStatus.md)

The status of the execution

### transactionDigest

> **transactionDigest**: `string`

The transaction digest

### unwrapped?

> `optional` **unwrapped**: [`OwnedObjectRef`](../interfaces/OwnedObjectRef.md)[]

ObjectRef and owner of objects that are unwrapped in this transaction. Unwrapped objects are objects
that were wrapped into other objects in the past, and just got extracted out.

### unwrappedThenDeleted?

> `optional` **unwrappedThenDeleted**: [`IotaObjectRef`](../interfaces/IotaObjectRef.md)[]

Object refs of objects previously wrapped in other objects but now deleted.

### wrapped?

> `optional` **wrapped**: [`IotaObjectRef`](../interfaces/IotaObjectRef.md)[]

Object refs of objects now wrapped in other objects.
