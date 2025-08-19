[**@iota/hierarchies API documentation**](../../api_ref.md)

---

# Class: CreateAccreditationToAccredit

A wrapper for the `CreateAccreditationToAccredit` transaction.

## Constructors

### Constructor

> **new CreateAccreditationToAccredit**(`federation_id`, `receiver_id`, `want_properties`, `owner`): `CreateAccreditationToAccredit`

Creates a new instance of `WasmCreateAccreditationToAccredit`.

# Arguments

- `federation_id` - The ID of the federation.
- `receiver_id` - The ID of the receiver of the accreditation.
- `want_properties` - The properties for which permissions are being granted.
- `owner` - The address of the transaction signer.

#### Parameters

##### federation\_id

`string`

##### receiver\_id

`string`

##### want\_properties

`any`[]

##### owner

`string`

#### Returns

`CreateAccreditationToAccredit`

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
