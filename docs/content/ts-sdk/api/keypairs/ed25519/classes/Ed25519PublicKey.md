# Class: Ed25519PublicKey

An Ed25519 public key

## Extends

- [`PublicKey`](../../../cryptography/classes/PublicKey.md)

## Constructors

### new Ed25519PublicKey()

> **new Ed25519PublicKey**(`value`): [`Ed25519PublicKey`](Ed25519PublicKey.md)

Create a new Ed25519PublicKey object

#### Parameters

• **value**: `PublicKeyInitData`

ed25519 public key as buffer or base-64 encoded string

#### Returns

[`Ed25519PublicKey`](Ed25519PublicKey.md)

#### Overrides

[`PublicKey`](../../../cryptography/classes/PublicKey.md).[`constructor`](../../../cryptography/classes/PublicKey.md#constructors)

## Properties

### SIZE

> `static` **SIZE**: `number` = `PUBLIC_KEY_SIZE`

## Methods

### toBase64()

> **toBase64**(): `string`

Return the base-64 representation of the public key

#### Returns

`string`

#### Inherited from

[`PublicKey`](../../../cryptography/classes/PublicKey.md).[`toBase64`](../../../cryptography/classes/PublicKey.md#tobase64)

---

### toString()

> **toString**(): `never`

#### Returns

`never`

#### Inherited from

[`PublicKey`](../../../cryptography/classes/PublicKey.md).[`toString`](../../../cryptography/classes/PublicKey.md#tostring)

---

### toIotaPublicKey()

> **toIotaPublicKey**(): `string`

Return the IOTA representation of the public key encoded in
base-64. A IOTA public key is formed by the concatenation
of the scheme flag with the raw bytes of the public key

#### Returns

`string`

#### Inherited from

[`PublicKey`](../../../cryptography/classes/PublicKey.md).[`toIotaPublicKey`](../../../cryptography/classes/PublicKey.md#toiotapublickey)

---

### verifyWithIntent()

> **verifyWithIntent**(`bytes`, `signature`, `intent`): `Promise`\<`boolean`\>

#### Parameters

• **bytes**: `Uint8Array`

• **signature**: `string` \| `Uint8Array`

• **intent**: [`IntentScope`](../../../cryptography/type-aliases/IntentScope.md)

#### Returns

`Promise`\<`boolean`\>

#### Inherited from

[`PublicKey`](../../../cryptography/classes/PublicKey.md).[`verifyWithIntent`](../../../cryptography/classes/PublicKey.md#verifywithintent)

---

### verifyPersonalMessage()

> **verifyPersonalMessage**(`message`, `signature`): `Promise`\<`boolean`\>

Verifies that the signature is valid for for the provided PersonalMessage

#### Parameters

• **message**: `Uint8Array`

• **signature**: `string` \| `Uint8Array`

#### Returns

`Promise`\<`boolean`\>

#### Inherited from

[`PublicKey`](../../../cryptography/classes/PublicKey.md).[`verifyPersonalMessage`](../../../cryptography/classes/PublicKey.md#verifypersonalmessage)

---

### verifyTransaction()

> **verifyTransaction**(`transaction`, `signature`): `Promise`\<`boolean`\>

Verifies that the signature is valid for for the provided Transaction

#### Parameters

• **transaction**: `Uint8Array`

• **signature**: `string` \| `Uint8Array`

#### Returns

`Promise`\<`boolean`\>

#### Inherited from

[`PublicKey`](../../../cryptography/classes/PublicKey.md).[`verifyTransaction`](../../../cryptography/classes/PublicKey.md#verifytransaction)

---

### toIotaBytes()

> **toIotaBytes**(): `Uint8Array`

Returns the bytes representation of the public key
prefixed with the signature scheme flag

#### Returns

`Uint8Array`

#### Inherited from

[`PublicKey`](../../../cryptography/classes/PublicKey.md).[`toIotaBytes`](../../../cryptography/classes/PublicKey.md#toiotabytes)

---

### toIotaBytesForAddress()

> **toIotaBytesForAddress**(): `Uint8Array`

Returns the bytes representation of the public key
prefixed with the signature scheme flag. If the
signature scheme is ED25519, no prefix is set.

#### Returns

`Uint8Array`

#### Inherited from

[`PublicKey`](../../../cryptography/classes/PublicKey.md).[`toIotaBytesForAddress`](../../../cryptography/classes/PublicKey.md#toiotabytesforaddress)

---

### toIotaAddress()

> **toIotaAddress**(): `string`

Return the IOTA address associated with this Ed25519 public key

#### Returns

`string`

#### Inherited from

[`PublicKey`](../../../cryptography/classes/PublicKey.md).[`toIotaAddress`](../../../cryptography/classes/PublicKey.md#toiotaaddress)

---

### equals()

> **equals**(`publicKey`): `boolean`

Checks if two Ed25519 public keys are equal

#### Parameters

• **publicKey**: [`Ed25519PublicKey`](Ed25519PublicKey.md)

#### Returns

`boolean`

#### Overrides

[`PublicKey`](../../../cryptography/classes/PublicKey.md).[`equals`](../../../cryptography/classes/PublicKey.md#equals)

---

### toRawBytes()

> **toRawBytes**(): `Uint8Array`

Return the byte array representation of the Ed25519 public key

#### Returns

`Uint8Array`

#### Overrides

[`PublicKey`](../../../cryptography/classes/PublicKey.md).[`toRawBytes`](../../../cryptography/classes/PublicKey.md#torawbytes)

---

### flag()

> **flag**(): `number`

Return the IOTA address associated with this Ed25519 public key

#### Returns

`number`

#### Overrides

[`PublicKey`](../../../cryptography/classes/PublicKey.md).[`flag`](../../../cryptography/classes/PublicKey.md#flag)

---

### verify()

> **verify**(`message`, `signature`): `Promise`\<`boolean`\>

Verifies that the signature is valid for for the provided message

#### Parameters

• **message**: `Uint8Array`

• **signature**: `string` \| `Uint8Array`

#### Returns

`Promise`\<`boolean`\>

#### Overrides

[`PublicKey`](../../../cryptography/classes/PublicKey.md).[`verify`](../../../cryptography/classes/PublicKey.md#verify)
