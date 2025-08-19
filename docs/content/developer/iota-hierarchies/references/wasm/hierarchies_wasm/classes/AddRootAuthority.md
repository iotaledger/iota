[**@iota/hierarchies API documentation**](../../api_ref.md)

***

# Class: AddRootAuthority

A wrapper for the `AddRootAuthority` transaction.

## Constructors

### Constructor

> **new AddRootAuthority**(`federation_id`, `account_id`, `signer_address`): `AddRootAuthority`

Creates a new instance of `WasmAddRootAuthority`.

# Arguments

* `federation_id` - The ID of the federation.
* `account_id` - The ID of the account to add as a root authority.
* `signer_address` - The address of the transaction signer.

#### Parameters

##### federation\_id

`string`

##### account\_id

`string`

##### signer\_address

`string`

#### Returns

`AddRootAuthority`

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

Builds and returns a programmable transaction for adding a root authority.

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

Applies transaction effects and events to this add root authority operation.

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
