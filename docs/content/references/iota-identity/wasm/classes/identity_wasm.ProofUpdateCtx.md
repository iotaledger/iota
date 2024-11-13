# Class: ProofUpdateCtx

[identity\_wasm](../modules/identity_wasm.md).ProofUpdateCtx

## Table of contents

### Properties

- [index\_end\_validity\_timeframe](identity_wasm.ProofUpdateCtx.md#index_end_validity_timeframe)
- [index\_start\_validity\_timeframe](identity_wasm.ProofUpdateCtx.md#index_start_validity_timeframe)
- [new\_end\_validity\_timeframe](identity_wasm.ProofUpdateCtx.md#new_end_validity_timeframe)
- [new\_start\_validity\_timeframe](identity_wasm.ProofUpdateCtx.md#new_start_validity_timeframe)
- [number\_of\_signed\_messages](identity_wasm.ProofUpdateCtx.md#number_of_signed_messages)
- [old\_end\_validity\_timeframe](identity_wasm.ProofUpdateCtx.md#old_end_validity_timeframe)
- [old\_start\_validity\_timeframe](identity_wasm.ProofUpdateCtx.md#old_start_validity_timeframe)

### Methods

- [toJSON](identity_wasm.ProofUpdateCtx.md#tojson)
- [toString](identity_wasm.ProofUpdateCtx.md#tostring)

## Properties

### index\_end\_validity\_timeframe

• **index\_end\_validity\_timeframe**: `number`

Index of `endValidityTimeframe` claim inside the array of Claims

___

### index\_start\_validity\_timeframe

• **index\_start\_validity\_timeframe**: `number`

Index of `startValidityTimeframe` claim inside the array of Claims

___

### new\_end\_validity\_timeframe

• **new\_end\_validity\_timeframe**: `Uint8Array`

New `endValidityTimeframe` value to be signed

___

### new\_start\_validity\_timeframe

• **new\_start\_validity\_timeframe**: `Uint8Array`

New `startValidityTimeframe` value to be signed

___

### number\_of\_signed\_messages

• **number\_of\_signed\_messages**: `number`

Number of signed messages, number of payloads in a JWP

___

### old\_end\_validity\_timeframe

• **old\_end\_validity\_timeframe**: `Uint8Array`

Old `endValidityTimeframe` value

___

### old\_start\_validity\_timeframe

• **old\_start\_validity\_timeframe**: `Uint8Array`

Old `startValidityTimeframe` value

## Methods

### toJSON

▸ **toJSON**(): `Object`

* Return copy of self without private attributes.

#### Returns

`Object`

___

### toString

▸ **toString**(): `string`

Return stringified version of self.

#### Returns

`string`
