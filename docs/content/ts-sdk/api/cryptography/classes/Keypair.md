# Class: `abstract` Keypair

TODO: Document

## Extends

- [`Signer`](Signer.md)

## Extended by

- [`Ed25519Keypair`](../../keypairs/ed25519/classes/Ed25519Keypair.md)
- [`Secp256k1Keypair`](../../keypairs/secp256k1/classes/Secp256k1Keypair.md)

## Constructors

### new Keypair()

> **new Keypair**(): [`Keypair`](Keypair.md)

#### Returns

[`Keypair`](Keypair.md)

#### Inherited from

[`Signer`](Signer.md).[`constructor`](Signer.md#constructors)

## Methods

### sign()

> `abstract` **sign**(`bytes`): `Promise`\<`Uint8Array`\>

#### Parameters

• **bytes**: `Uint8Array`

#### Returns

`Promise`\<`Uint8Array`\>

#### Inherited from

[`Signer`](Signer.md).[`sign`](Signer.md#sign)

---

### signWithIntent()

> **signWithIntent**(`bytes`, `intent`): `Promise`\<[`SignatureWithBytes`](../interfaces/SignatureWithBytes.md)\>

Sign messages with a specific intent. By combining the message bytes with the intent before hashing and signing,
it ensures that a signed message is tied to a specific purpose and domain separator is provided

#### Parameters

• **bytes**: `Uint8Array`

• **intent**: [`IntentScope`](../type-aliases/IntentScope.md)

#### Returns

`Promise`\<[`SignatureWithBytes`](../interfaces/SignatureWithBytes.md)\>

#### Inherited from

[`Signer`](Signer.md).[`signWithIntent`](Signer.md#signwithintent)

---

### signTransaction()

> **signTransaction**(`bytes`): `Promise`\<[`SignatureWithBytes`](../interfaces/SignatureWithBytes.md)\>

Signs provided transaction by calling `signWithIntent()` with a `TransactionData` provided as intent scope

#### Parameters

• **bytes**: `Uint8Array`

#### Returns

`Promise`\<[`SignatureWithBytes`](../interfaces/SignatureWithBytes.md)\>

#### Inherited from

[`Signer`](Signer.md).[`signTransaction`](Signer.md#signtransaction)

---

### signPersonalMessage()

> **signPersonalMessage**(`bytes`): `Promise`\<`object`\>

Signs provided personal message by calling `signWithIntent()` with a `PersonalMessage` provided as intent scope

#### Parameters

• **bytes**: `Uint8Array`

#### Returns

`Promise`\<`object`\>

##### bytes

> **bytes**: `string`

##### signature

> **signature**: `string`

#### Inherited from

[`Signer`](Signer.md).[`signPersonalMessage`](Signer.md#signpersonalmessage)

---

### toIotaAddress()

> **toIotaAddress**(): `string`

#### Returns

`string`

#### Inherited from

[`Signer`](Signer.md).[`toIotaAddress`](Signer.md#toiotaaddress)

---

### getKeyScheme()

> `abstract` **getKeyScheme**(): [`SignatureScheme`](../type-aliases/SignatureScheme.md)

Get the key scheme of the keypair: Secp256k1 or ED25519

#### Returns

[`SignatureScheme`](../type-aliases/SignatureScheme.md)

#### Inherited from

[`Signer`](Signer.md).[`getKeyScheme`](Signer.md#getkeyscheme)

---

### getPublicKey()

> `abstract` **getPublicKey**(): [`PublicKey`](PublicKey.md)

The public key for this keypair

#### Returns

[`PublicKey`](PublicKey.md)

#### Inherited from

[`Signer`](Signer.md).[`getPublicKey`](Signer.md#getpublickey)

---

### getSecretKey()

> `abstract` **getSecretKey**(): `string`

This returns the Bech32 secret key string for this keypair.

#### Returns

`string`
