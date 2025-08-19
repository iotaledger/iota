[**@iota/hierarchies API documentation**](../../api_ref.md)

---

# Class: CreateAccreditationToAttest

A wrapper for the `CreateAccreditationToAttest` transaction.

## Constructors

### Constructor

> **new CreateAccreditationToAttest**(`federation_id`, `receiver`, `want_properties`, `owner`): `CreateAccreditationToAttest`

Creates a new instance of `WasmCreateAccreditationToAttest`.

# Arguments

- `federation_id` - The ID of the federation.
- `receiver` - The ID of the receiver of the accreditation.
- `want_properties` - The properties for which permissions are being granted.
- `owner` - The address of the transaction signer.

#### Parameters

##### federation\_id

`string`

##### receiver

`string`

##### want\_properties

`any`[]

##### owner

`string`

#### Returns

`CreateAccreditationToAttest`

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

Builds and returns a programmable transaction for creating an accreditation to accredit.

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

Applies transaction effects and events to this create accreditation to accredit operation.

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
