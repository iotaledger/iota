# Class: DecodedJws

[identity\_wasm](../modules/identity_wasm.md).DecodedJws

A cryptographically verified decoded token from a JWS.

Contains the decoded headers and the raw claims.

## Table of contents

### Methods

- [toJSON](identity_wasm.DecodedJws.md#tojson)
- [toString](identity_wasm.DecodedJws.md#tostring)
- [claims](identity_wasm.DecodedJws.md#claims)
- [claimsBytes](identity_wasm.DecodedJws.md#claimsbytes)
- [protectedHeader](identity_wasm.DecodedJws.md#protectedheader)
- [clone](identity_wasm.DecodedJws.md#clone)

## Methods

### toJSON

▸ **toJSON**(): `Object`

* Return copy of self without private attributes.

#### Returns

`Object`

▸ **toJSON**(): `any`

Serializes this to a JSON object.

#### Returns

`any`

___

### toString

▸ **toString**(): `string`

Return stringified version of self.

#### Returns

`string`

___

### claims

▸ **claims**(): `string`

Returns a copy of the parsed claims represented as a string.

# Errors
An error is thrown if the claims cannot be represented as a string.

This error can only occur if the Token was decoded from a detached payload.

#### Returns

`string`

___

### claimsBytes

▸ **claimsBytes**(): `Uint8Array`

Return a copy of the parsed claims represented as an array of bytes.

#### Returns

`Uint8Array`

___

### protectedHeader

▸ **protectedHeader**(): [`JwsHeader`](identity_wasm.JwsHeader.md)

Returns a copy of the protected header.

#### Returns

[`JwsHeader`](identity_wasm.JwsHeader.md)

___

### clone

▸ **clone**(): [`DecodedJws`](identity_wasm.DecodedJws.md)

Deep clones the object.

#### Returns

[`DecodedJws`](identity_wasm.DecodedJws.md)
