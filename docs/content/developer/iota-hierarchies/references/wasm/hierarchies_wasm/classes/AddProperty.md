[**@iota/hierarchies API documentation**](../../api_ref.md)

---

# Class: AddProperty

A wrapper for the `AddProperty` transaction.

## Constructors

### Constructor

> **new AddProperty**(`federation_id`, `property`, `owner`): `AddProperty`

Creates a new instance of `WasmAddProperty`.

# Arguments

- `federation_id` - The ID of the federation.
- `property` - The property to add.
- `owner` - The address of the transaction signer.

#### Parameters

##### federation\_id

`string`

##### property

[`FederationProperty`](FederationProperty.md)

##### owner

`string`

#### Returns

`AddProperty`

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

Builds and returns a programmable transaction for adding a property.

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

> **applyWithEvents**(`wasm_effects`, `wasm_events`, `client`): `Promise`\<`void`\>

Applies transaction effects and events to this add property operation.

# Arguments

- `effects` - The transaction block effects to apply.
- `events` - The transaction block events to apply.
- `client` - A read-only client for blockchain interaction.

#### Parameters

##### wasm\_effects

`TransactionEffects`

##### wasm\_events

`IotaEvent`[]

##### client

`CoreClientReadOnly`

#### Returns

`Promise`\<`void`\>
