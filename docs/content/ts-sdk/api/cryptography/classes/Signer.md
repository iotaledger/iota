# Class: `abstract` Signer

TODO: Document

## Extended by

- [`Keypair`](Keypair.md)
- [`MultiSigSigner`](../../multisig/classes/MultiSigSigner.md)

## Constructors

### new Signer()

> **new Signer**(): [`Signer`](Signer.md)

#### Returns

[`Signer`](Signer.md)

## Methods

### sign()

> `abstract` **sign**(`bytes`): `Promise`\<`Uint8Array`\>

#### Parameters

• **bytes**: `Uint8Array`

#### Returns

`Promise`\<`Uint8Array`\>

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

---

### signTransaction()

> **signTransaction**(`bytes`): `Promise`\<[`SignatureWithBytes`](../interfaces/SignatureWithBytes.md)\>

Signs provided transaction by calling `signWithIntent()` with a `TransactionData` provided as intent scope

#### Parameters

• **bytes**: `Uint8Array`

#### Returns

`Promise`\<[`SignatureWithBytes`](../interfaces/SignatureWithBytes.md)\>

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

---

### toIotaAddress()

> **toIotaAddress**(): `string`

#### Returns

`string`

---

### getKeyScheme()

> `abstract` **getKeyScheme**(): [`SignatureScheme`](../type-aliases/SignatureScheme.md)

Get the key scheme of the keypair: Secp256k1 or ED25519

#### Returns

[`SignatureScheme`](../type-aliases/SignatureScheme.md)

---

### getPublicKey()

> `abstract` **getPublicKey**(): [`PublicKey`](PublicKey.md)

The public key for this keypair

#### Returns

[`PublicKey`](PublicKey.md)
