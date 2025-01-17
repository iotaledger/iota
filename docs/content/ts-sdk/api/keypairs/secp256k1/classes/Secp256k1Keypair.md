# Class: Secp256k1Keypair

An Secp256k1 Keypair used for signing transactions.

## Extends

- [`Keypair`](../../../cryptography/classes/Keypair.md)

## Constructors

### new Secp256k1Keypair()

> **new Secp256k1Keypair**(`keypair`?): [`Secp256k1Keypair`](Secp256k1Keypair.md)

Create a new keypair instance.
Generate random keypair if no [Secp256k1Keypair](Secp256k1Keypair.md) is provided.

#### Parameters

• **keypair?**: [`Secp256k1KeypairData`](../interfaces/Secp256k1KeypairData.md)

secp256k1 keypair

#### Returns

[`Secp256k1Keypair`](Secp256k1Keypair.md)

#### Overrides

[`Keypair`](../../../cryptography/classes/Keypair.md).[`constructor`](../../../cryptography/classes/Keypair.md#constructors)

## Methods

### signWithIntent()

> **signWithIntent**(`bytes`, `intent`): `Promise`\<[`SignatureWithBytes`](../../../cryptography/interfaces/SignatureWithBytes.md)\>

Sign messages with a specific intent. By combining the message bytes with the intent before hashing and signing,
it ensures that a signed message is tied to a specific purpose and domain separator is provided

#### Parameters

• **bytes**: `Uint8Array`

• **intent**: [`IntentScope`](../../../cryptography/type-aliases/IntentScope.md)

#### Returns

`Promise`\<[`SignatureWithBytes`](../../../cryptography/interfaces/SignatureWithBytes.md)\>

#### Inherited from

[`Keypair`](../../../cryptography/classes/Keypair.md).[`signWithIntent`](../../../cryptography/classes/Keypair.md#signwithintent)

---

### signTransaction()

> **signTransaction**(`bytes`): `Promise`\<[`SignatureWithBytes`](../../../cryptography/interfaces/SignatureWithBytes.md)\>

Signs provided transaction by calling `signWithIntent()` with a `TransactionData` provided as intent scope

#### Parameters

• **bytes**: `Uint8Array`

#### Returns

`Promise`\<[`SignatureWithBytes`](../../../cryptography/interfaces/SignatureWithBytes.md)\>

#### Inherited from

[`Keypair`](../../../cryptography/classes/Keypair.md).[`signTransaction`](../../../cryptography/classes/Keypair.md#signtransaction)

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

[`Keypair`](../../../cryptography/classes/Keypair.md).[`signPersonalMessage`](../../../cryptography/classes/Keypair.md#signpersonalmessage)

---

### toIotaAddress()

> **toIotaAddress**(): `string`

#### Returns

`string`

#### Inherited from

[`Keypair`](../../../cryptography/classes/Keypair.md).[`toIotaAddress`](../../../cryptography/classes/Keypair.md#toiotaaddress)

---

### getKeyScheme()

> **getKeyScheme**(): [`SignatureScheme`](../../../cryptography/type-aliases/SignatureScheme.md)

Get the key scheme of the keypair Secp256k1

#### Returns

[`SignatureScheme`](../../../cryptography/type-aliases/SignatureScheme.md)

#### Overrides

[`Keypair`](../../../cryptography/classes/Keypair.md).[`getKeyScheme`](../../../cryptography/classes/Keypair.md#getkeyscheme)

---

### generate()

> `static` **generate**(): [`Secp256k1Keypair`](Secp256k1Keypair.md)

Generate a new random keypair

#### Returns

[`Secp256k1Keypair`](Secp256k1Keypair.md)

---

### fromSecretKey()

> `static` **fromSecretKey**(`secretKey`, `options`?): [`Secp256k1Keypair`](Secp256k1Keypair.md)

Create a keypair from a raw secret key byte array.

This method should only be used to recreate a keypair from a previously
generated secret key. Generating keypairs from a random seed should be done
with the Keypair.fromSeed method.

#### Parameters

• **secretKey**: `Uint8Array`

secret key byte array

• **options?**

• **options.skipValidation?**: `boolean`

#### Returns

[`Secp256k1Keypair`](Secp256k1Keypair.md)

#### Throws

error if the provided secret key is invalid and validation is not skipped.

---

### fromSeed()

> `static` **fromSeed**(`seed`): [`Secp256k1Keypair`](Secp256k1Keypair.md)

Generate a keypair from a 32 byte seed.

#### Parameters

• **seed**: `Uint8Array`

seed byte array

#### Returns

[`Secp256k1Keypair`](Secp256k1Keypair.md)

---

### getPublicKey()

> **getPublicKey**(): [`PublicKey`](../../../cryptography/classes/PublicKey.md)

The public key for this keypair

#### Returns

[`PublicKey`](../../../cryptography/classes/PublicKey.md)

#### Overrides

[`Keypair`](../../../cryptography/classes/Keypair.md).[`getPublicKey`](../../../cryptography/classes/Keypair.md#getpublickey)

---

### getSecretKey()

> **getSecretKey**(): `string`

The Bech32 secret key string for this Secp256k1 keypair

#### Returns

`string`

#### Overrides

[`Keypair`](../../../cryptography/classes/Keypair.md).[`getSecretKey`](../../../cryptography/classes/Keypair.md#getsecretkey)

---

### sign()

> **sign**(`data`): `Promise`\<`Uint8Array`\>

Return the signature for the provided data.

#### Parameters

• **data**: `Uint8Array`

#### Returns

`Promise`\<`Uint8Array`\>

#### Overrides

[`Keypair`](../../../cryptography/classes/Keypair.md).[`sign`](../../../cryptography/classes/Keypair.md#sign)

---

### deriveKeypair()

> `static` **deriveKeypair**(`mnemonics`, `path`?): [`Secp256k1Keypair`](Secp256k1Keypair.md)

Derive Secp256k1 keypair from mnemonics and path. The mnemonics must be normalized
and validated against the english wordlist.

If path is none, it will default to m/54'/4218'/0'/0/0, otherwise the path must
be compliant to BIP-32 in form m/54'/4218'/{account_index}'/{change_index}/{address_index}.

#### Parameters

• **mnemonics**: `string`

• **path?**: `string`

#### Returns

[`Secp256k1Keypair`](Secp256k1Keypair.md)
