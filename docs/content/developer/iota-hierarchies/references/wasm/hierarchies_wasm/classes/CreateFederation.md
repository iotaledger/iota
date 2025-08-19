[**@iota/hierarchies API documentation**](../../api_ref.md)

---

# Class: CreateFederation

A wrapper for the `CreateFederation` transaction.

## Constructors

### Constructor

> **new CreateFederation**(): `CreateFederation`

Creates a new instance of `WasmCreateFederation`.

#### Returns

`CreateFederation`

## Methods

### toJSON()

> **toJSON**(): `Object`

- Return copy of self without private attributes.

#### Returns

`Object`

---

### toString()

> **toString**(): `string`

Return stringified version of self.

#### Returns

`string`

---

### buildProgrammableTransaction()

> **buildProgrammableTransaction**(`client`): `Promise`\<`Uint8Array`\<`ArrayBufferLike`\>\>

Builds and returns a programmable transaction for creating a new federation.

# Arguments

- `client` - A read-only client for blockchain interaction.

# Returns

The binary BCS serialization of the programmable transaction.

# Errors

Returns an error if the transaction cannot be built.

#### Parameters

##### client

`CoreClientReadOnly`

#### Returns

`Promise`\<`Uint8Array`\<`ArrayBufferLike`\>\>

---

### applyWithEvents()

> **applyWithEvents**(`wasm_effects`, `wasm_events`, `client`): `Promise`\<[`Federation`](Federation.md)\>

Applies transaction effects and events to this create federation operation.

# Arguments

- `effects` - The transaction block effects to apply.
- `events` - The transaction block events to apply.
- `client` - A read-only client for blockchain interaction.

# Returns

A `WasmFederation` object representing the newly created federation.

#### Parameters

##### wasm\_effects

`TransactionEffects`

##### wasm\_events

`IotaEvent`[]

##### client

`CoreClientReadOnly`

#### Returns

`Promise`\<[`Federation`](Federation.md)\>
