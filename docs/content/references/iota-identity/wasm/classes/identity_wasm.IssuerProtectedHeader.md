# Class: IssuerProtectedHeader

[identity\_wasm](../modules/identity_wasm.md).IssuerProtectedHeader

## Table of contents

### Properties

- [alg](identity_wasm.IssuerProtectedHeader.md#alg)
- [cid](identity_wasm.IssuerProtectedHeader.md#cid)
- [kid](identity_wasm.IssuerProtectedHeader.md#kid)
- [typ](identity_wasm.IssuerProtectedHeader.md#typ)

### Methods

- [toJSON](identity_wasm.IssuerProtectedHeader.md#tojson)
- [toString](identity_wasm.IssuerProtectedHeader.md#tostring)
- [claims](identity_wasm.IssuerProtectedHeader.md#claims)

## Properties

### alg

• **alg**: [`ProofAlgorithm`](../enums/identity_wasm.ProofAlgorithm.md)

Algorithm used for the JWP.

___

### cid

• `Optional` **cid**: `string`

Not handled for now. Will be used in the future to resolve external claims

___

### kid

• `Optional` **kid**: `string`

ID for the key used for the JWP.

___

### typ

• `Optional` **typ**: `string`

JWP type (JPT).

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

___

### claims

▸ **claims**(): `string`[]

#### Returns

`string`[]
