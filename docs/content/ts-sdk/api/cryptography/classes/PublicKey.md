# Class: `abstract` PublicKey

A public key

## Extended by

- [`Ed25519PublicKey`](../../keypairs/ed25519/classes/Ed25519PublicKey.md)
- [`Secp256k1PublicKey`](../../keypairs/secp256k1/classes/Secp256k1PublicKey.md)
- [`MultiSigPublicKey`](../../multisig/classes/MultiSigPublicKey.md)

## Constructors

### new PublicKey()

> **new PublicKey**(): [`PublicKey`](PublicKey.md)

#### Returns

[`PublicKey`](PublicKey.md)

## Methods

### equals()

> **equals**(`publicKey`): `boolean`

Checks if two public keys are equal

#### Parameters

• **publicKey**: [`PublicKey`](PublicKey.md)

#### Returns

`boolean`

---

### toBase64()

> **toBase64**(): `string`

Return the base-64 representation of the public key

#### Returns

`string`

---

### toString()

> **toString**(): `never`

#### Returns

`never`

---

### toIotaPublicKey()

> **toIotaPublicKey**(): `string`

Return the IOTA representation of the public key encoded in
base-64. An IOTA public key is formed by the concatenation
of the scheme flag with the raw bytes of the public key

#### Returns

`string`

---

### verifyWithIntent()

> **verifyWithIntent**(`bytes`, `signature`, `intent`): `Promise`\<`boolean`\>

#### Parameters

• **bytes**: `Uint8Array`

• **signature**: `string` \| `Uint8Array`

• **intent**: [`IntentScope`](../type-aliases/IntentScope.md)

#### Returns

`Promise`\<`boolean`\>

---

### verifyPersonalMessage()

> **verifyPersonalMessage**(`message`, `signature`): `Promise`\<`boolean`\>

Verifies that the signature is valid for the provided PersonalMessage

#### Parameters

• **message**: `Uint8Array`

• **signature**: `string` \| `Uint8Array`

#### Returns

`Promise`\<`boolean`\>

---

### verifyTransaction()

> **verifyTransaction**(`transaction`, `signature`): `Promise`\<`boolean`\>

Verifies that the signature is valid for the provided Transaction

#### Parameters

• **transaction**: `Uint8Array`

• **signature**: `string` \| `Uint8Array`

#### Returns

`Promise`\<`boolean`\>

---

### toIotaBytes()

> **toIotaBytes**(): `Uint8Array`

Returns the bytes representation of the public key
prefixed with the signature scheme flag

#### Returns

`Uint8Array`

---

### toIotaBytesForAddress()

> **toIotaBytesForAddress**(): `Uint8Array`

Returns the bytes representation of the public key
prefixed with the signature scheme flag. If the
signature scheme is ED25519, no prefix is set.

#### Returns

`Uint8Array`

---

### toIotaAddress()

> **toIotaAddress**(): `string`

Return the IOTA address associated with this Ed25519 public key

#### Returns

`string`

---

### toRawBytes()

> `abstract` **toRawBytes**(): `Uint8Array`

Return the byte array representation of the public key

#### Returns

`Uint8Array`

---

### flag()

> `abstract` **flag**(): `number`

Return signature scheme flag of the public key

#### Returns

`number`

---

### verify()

> `abstract` **verify**(`data`, `signature`): `Promise`\<`boolean`\>

Verifies that the signature is valid for the provided message

#### Parameters

• **data**: `Uint8Array`

• **signature**: `string` \| `Uint8Array`

#### Returns

`Promise`\<`boolean`\>
