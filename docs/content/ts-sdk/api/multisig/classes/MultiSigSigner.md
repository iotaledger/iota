# Class: MultiSigSigner

TODO: Document

## Extends

- [`Signer`](../../cryptography/classes/Signer.md)

## Constructors

### new MultiSigSigner()

> **new MultiSigSigner**(`pubkey`, `signers`): [`MultiSigSigner`](MultiSigSigner.md)

#### Parameters

• **pubkey**: [`MultiSigPublicKey`](MultiSigPublicKey.md)

• **signers**: [`Signer`](../../cryptography/classes/Signer.md)[] = `[]`

#### Returns

[`MultiSigSigner`](MultiSigSigner.md)

#### Overrides

[`Signer`](../../cryptography/classes/Signer.md).[`constructor`](../../cryptography/classes/Signer.md#constructors)

## Methods

### signWithIntent()

> **signWithIntent**(`bytes`, `intent`): `Promise`\<[`SignatureWithBytes`](../../cryptography/interfaces/SignatureWithBytes.md)\>

Sign messages with a specific intent. By combining the message bytes with the intent before hashing and signing,
it ensures that a signed message is tied to a specific purpose and domain separator is provided

#### Parameters

• **bytes**: `Uint8Array`

• **intent**: [`IntentScope`](../../cryptography/type-aliases/IntentScope.md)

#### Returns

`Promise`\<[`SignatureWithBytes`](../../cryptography/interfaces/SignatureWithBytes.md)\>

#### Inherited from

[`Signer`](../../cryptography/classes/Signer.md).[`signWithIntent`](../../cryptography/classes/Signer.md#signwithintent)

---

### toIotaAddress()

> **toIotaAddress**(): `string`

#### Returns

`string`

#### Inherited from

[`Signer`](../../cryptography/classes/Signer.md).[`toIotaAddress`](../../cryptography/classes/Signer.md#toiotaaddress)

---

### getKeyScheme()

> **getKeyScheme**(): [`SignatureScheme`](../../cryptography/type-aliases/SignatureScheme.md)

Get the key scheme of the keypair: Secp256k1 or ED25519

#### Returns

[`SignatureScheme`](../../cryptography/type-aliases/SignatureScheme.md)

#### Overrides

[`Signer`](../../cryptography/classes/Signer.md).[`getKeyScheme`](../../cryptography/classes/Signer.md#getkeyscheme)

---

### getPublicKey()

> **getPublicKey**(): [`MultiSigPublicKey`](MultiSigPublicKey.md)

The public key for this keypair

#### Returns

[`MultiSigPublicKey`](MultiSigPublicKey.md)

#### Overrides

[`Signer`](../../cryptography/classes/Signer.md).[`getPublicKey`](../../cryptography/classes/Signer.md#getpublickey)

---

### sign()

> **sign**(`_data`): `never`

#### Parameters

• **\_data**: `Uint8Array`

#### Returns

`never`

#### Overrides

[`Signer`](../../cryptography/classes/Signer.md).[`sign`](../../cryptography/classes/Signer.md#sign)

---

### signData()

> **signData**(`_data`): `never`

#### Parameters

• **\_data**: `Uint8Array`

#### Returns

`never`

---

### signTransaction()

> **signTransaction**(`bytes`): `Promise`\<`object`\>

Signs provided transaction by calling `signWithIntent()` with a `TransactionData` provided as intent scope

#### Parameters

• **bytes**: `Uint8Array`

#### Returns

`Promise`\<`object`\>

##### signature

> **signature**: `string`

##### bytes

> **bytes**: `string`

#### Overrides

[`Signer`](../../cryptography/classes/Signer.md).[`signTransaction`](../../cryptography/classes/Signer.md#signtransaction)

---

### signPersonalMessage()

> **signPersonalMessage**(`bytes`): `Promise`\<`object`\>

Signs provided personal message by calling `signWithIntent()` with a `PersonalMessage` provided as intent scope

#### Parameters

• **bytes**: `Uint8Array`

#### Returns

`Promise`\<`object`\>

##### signature

> **signature**: `string`

##### bytes

> **bytes**: `string`

#### Overrides

[`Signer`](../../cryptography/classes/Signer.md).[`signPersonalMessage`](../../cryptography/classes/Signer.md#signpersonalmessage)
