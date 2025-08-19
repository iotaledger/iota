[**@iota/hierarchies API documentation**](../../api_ref.md)

***

# Class: RevokeAccreditationToAccredit

A wrapper for the `RevokeAccreditationToAccredit` transaction.

## Constructors

### Constructor

> **new RevokeAccreditationToAccredit**(`federation_id`, `entity_id`, `accreditation_id`, `owner`): `RevokeAccreditationToAccredit`

Creates a new instance of `WasmRevokeAccreditationToAccredit`.

# Arguments

* `federation_id` - The ID of the federation.
* `entity_id` - The ID of entity whose accreditation is being revoked.
* `accreditation_id` - The ID of the accreditation to revoke.
* `owner` - The address of the transaction signer.

#### Parameters

##### federation\_id

`string`

##### entity\_id

`string`

##### accreditation\_id

`string`

##### owner

`string`

#### Returns

`RevokeAccreditationToAccredit`

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

Builds and returns a programmable transaction for revoking an accreditation to accredit.

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

Applies transaction effects and events to this revoke accreditation to accredit operation.

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
