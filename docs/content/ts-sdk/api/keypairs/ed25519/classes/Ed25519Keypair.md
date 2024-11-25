# Class: Ed25519Keypair

An Ed25519 Keypair used for signing transactions.

## Extends

- [`Keypair`](../../../cryptography/classes/Keypair.md)

## Constructors

### new Ed25519Keypair()

> **new Ed25519Keypair**(`keypair`?): [`Ed25519Keypair`](Ed25519Keypair.md)

Create a new Ed25519 keypair instance.
Generate random keypair if no [Ed25519Keypair](Ed25519Keypair.md) is provided.

#### Parameters

• **keypair?**: [`Ed25519KeypairData`](../interfaces/Ed25519KeypairData.md)

Ed25519 keypair

#### Returns

[`Ed25519Keypair`](Ed25519Keypair.md)

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

Get the key scheme of the keypair ED25519

#### Returns

[`SignatureScheme`](../../../cryptography/type-aliases/SignatureScheme.md)

#### Overrides

[`Keypair`](../../../cryptography/classes/Keypair.md).[`getKeyScheme`](../../../cryptography/classes/Keypair.md#getkeyscheme)

---

### generate()

> `static` **generate**(): [`Ed25519Keypair`](Ed25519Keypair.md)

Generate a new random Ed25519 keypair

#### Returns

[`Ed25519Keypair`](Ed25519Keypair.md)

---

### fromSecretKey()

> `static` **fromSecretKey**(`secretKey`, `options`?): [`Ed25519Keypair`](Ed25519Keypair.md)

Create a Ed25519 keypair from a raw secret key byte array, also known as seed.
This is NOT the private scalar which is result of hashing and bit clamping of
the raw secret key.

#### Parameters

• **secretKey**: `Uint8Array`

secret key byte array

• **options?**

• **options.skipValidation?**: `boolean`

#### Returns

[`Ed25519Keypair`](Ed25519Keypair.md)

#### Throws

error if the provided secret key is invalid and validation is not skipped.

---

### getPublicKey()

> **getPublicKey**(): [`Ed25519PublicKey`](Ed25519PublicKey.md)

The public key for this Ed25519 keypair

#### Returns

[`Ed25519PublicKey`](Ed25519PublicKey.md)

#### Overrides

[`Keypair`](../../../cryptography/classes/Keypair.md).[`getPublicKey`](../../../cryptography/classes/Keypair.md#getpublickey)

---

### getSecretKey()

> **getSecretKey**(): `string`

The Bech32 secret key string for this Ed25519 keypair

#### Returns

`string`

#### Overrides

[`Keypair`](../../../cryptography/classes/Keypair.md).[`getSecretKey`](../../../cryptography/classes/Keypair.md#getsecretkey)

---

### sign()

> **sign**(`data`): `Promise`\<`Uint8Array`\>

Return the signature for the provided data using Ed25519.

#### Parameters

• **data**: `Uint8Array`

#### Returns

`Promise`\<`Uint8Array`\>

#### Overrides

[`Keypair`](../../../cryptography/classes/Keypair.md).[`sign`](../../../cryptography/classes/Keypair.md#sign)

---

### deriveKeypair()

> `static` **deriveKeypair**(`mnemonics`, `path`?): [`Ed25519Keypair`](Ed25519Keypair.md)

Derive Ed25519 keypair from mnemonics and path. The mnemonics must be normalized
and validated against the english wordlist.

If path is none, it will default to m/44'/4218'/0'/0'/0', otherwise the path must
be compliant to SLIP-0010 in form m/44'/4218'/{account_index}'/{change_index}'/{address_index}'.

#### Parameters

• **mnemonics**: `string`

• **path?**: `string`

#### Returns

[`Ed25519Keypair`](Ed25519Keypair.md)

---

### deriveKeypairFromSeed()

> `static` **deriveKeypairFromSeed**(`seedHex`, `path`?): [`Ed25519Keypair`](Ed25519Keypair.md)

Derive Ed25519 keypair from mnemonicSeed and path.

If path is none, it will default to m/44'/4218'/0'/0'/0', otherwise the path must
be compliant to SLIP-0010 in form m/44'/4218'/{account_index}'/{change_index}'/{address_index}'.

#### Parameters

• **seedHex**: `string`

• **path?**: `string`

#### Returns

[`Ed25519Keypair`](Ed25519Keypair.md)
