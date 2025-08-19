[**@iota/hierarchies API documentation**](../../api_ref.md)

***

# Class: RevokeProperty

A wrapper for the `RevokeProperty` transaction.

## Constructors

### Constructor

> **new RevokeProperty**(`federation_id`, `property_name`, `valid_to_ms`, `owner`): `RevokeProperty`

Creates a new instance of `WasmRevokeProperty`.

# Arguments

* `federation_id` - The ID of the federation.
* `property_name` - The name of the property to revoke.
* `valid_to_ms` - The timestamp until which the property is valid.
* `owner` - The address of the transaction signer.

#### Parameters

##### federation\_id

`string`

##### property\_name

`PropertyName`

##### valid\_to\_ms

`undefined` | `null` | `bigint`

##### owner

`string`

#### Returns

`RevokeProperty`

## Methods

### toJSON()

> **toJSON**(): `Object`

* Return copy of self without private attributes.

#### Returns

`Object`

***

### toString()

> **toString**(): `string`

Return stringified version of self.

#### Returns

`string`

***

### buildProgrammableTransaction()

> **buildProgrammableTransaction**(`client`): `Promise`\<`Uint8Array`\<`ArrayBufferLike`\>\>

Builds and returns a programmable transaction for revoking a property.

# Arguments

* `client` - A read-only client for blockchain interaction.

# Returns

The binary BCS serialization of the programmable transaction.

# Errors

Returns an error if the transaction cannot be built.

#### Parameters

##### client

`CoreClientReadOnly`

#### Returns

`Promise`\<`Uint8Array`\<`ArrayBufferLike`\>\>

***

### applyWithEvents()

> **applyWithEvents**(`wasm_effects`, `wasm_events`, `client`): `Promise`\<`void`\>

Applies transaction effects and events to this revoke property operation.

# Arguments

* `effects` - The transaction block effects to apply.
* `events` - The transaction block events to apply.
* `client` - A read-only client for blockchain interaction.

#### Parameters

##### wasm\_effects

`TransactionEffects`

##### wasm\_events

`IotaEvent`[]

##### client

`CoreClientReadOnly`

#### Returns

`Promise`\<`void`\>
