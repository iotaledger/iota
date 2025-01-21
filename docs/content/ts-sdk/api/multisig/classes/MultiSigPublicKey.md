# Class: MultiSigPublicKey

A MultiSig public key

## Extends

- [`PublicKey`](../../cryptography/classes/PublicKey.md)

## Constructors

### new MultiSigPublicKey()

> **new MultiSigPublicKey**(`value`): [`MultiSigPublicKey`](MultiSigPublicKey.md)

Create a new MultiSigPublicKey object

#### Parameters

• **value**: `string` \| `Uint8Array` \| `MultiSigPublicKeyStruct`

MultiSig public key as buffer or base-64 encoded string

#### Returns

[`MultiSigPublicKey`](MultiSigPublicKey.md)

#### Overrides

[`PublicKey`](../../cryptography/classes/PublicKey.md).[`constructor`](../../cryptography/classes/PublicKey.md#constructors)

## Methods

### toBase64()

> **toBase64**(): `string`

Return the base-64 representation of the public key

#### Returns

`string`

#### Inherited from

[`PublicKey`](../../cryptography/classes/PublicKey.md).[`toBase64`](../../cryptography/classes/PublicKey.md#tobase64)

---

### toString()

> **toString**(): `never`

#### Returns

`never`

#### Inherited from

[`PublicKey`](../../cryptography/classes/PublicKey.md).[`toString`](../../cryptography/classes/PublicKey.md#tostring)

---

### toIotaPublicKey()

> **toIotaPublicKey**(): `string`

Return the IOTA representation of the public key encoded in
base-64. An IOTA public key is formed by the concatenation
of the scheme flag with the raw bytes of the public key

#### Returns

`string`

#### Inherited from

[`PublicKey`](../../cryptography/classes/PublicKey.md).[`toIotaPublicKey`](../../cryptography/classes/PublicKey.md#toiotapublickey)

---

### verifyWithIntent()

> **verifyWithIntent**(`bytes`, `signature`, `intent`): `Promise`\<`boolean`\>

#### Parameters

• **bytes**: `Uint8Array`

• **signature**: `string` \| `Uint8Array`

• **intent**: [`IntentScope`](../../cryptography/type-aliases/IntentScope.md)

#### Returns

`Promise`\<`boolean`\>

#### Inherited from

[`PublicKey`](../../cryptography/classes/PublicKey.md).[`verifyWithIntent`](../../cryptography/classes/PublicKey.md#verifywithintent)

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

[`PublicKey`](../../cryptography/classes/PublicKey.md).[`verifyPersonalMessage`](../../cryptography/classes/PublicKey.md#verifypersonalmessage)

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

[`PublicKey`](../../cryptography/classes/PublicKey.md).[`verifyTransaction`](../../cryptography/classes/PublicKey.md#verifytransaction)

---

### toIotaBytes()

> **toIotaBytes**(): `Uint8Array`

Returns the bytes representation of the public key
prefixed with the signature scheme flag

#### Returns

`Uint8Array`

#### Inherited from

[`PublicKey`](../../cryptography/classes/PublicKey.md).[`toIotaBytes`](../../cryptography/classes/PublicKey.md#toiotabytes)

---

### toIotaBytesForAddress()

> **toIotaBytesForAddress**(): `Uint8Array`

Returns the bytes representation of the public key
prefixed with the signature scheme flag. If the
signature scheme is ED25519, no prefix is set.

#### Returns

`Uint8Array`

#### Inherited from

[`PublicKey`](../../cryptography/classes/PublicKey.md).[`toIotaBytesForAddress`](../../cryptography/classes/PublicKey.md#toiotabytesforaddress)

---

### fromPublicKeys()

> `static` **fromPublicKeys**(`__namedParameters`): [`MultiSigPublicKey`](MultiSigPublicKey.md)

A static method to create a new MultiSig publickey instance from a set of public keys and their associated weights pairs and threshold.

#### Parameters

• **\_\_namedParameters**

• **\_\_namedParameters.threshold**: `number`

• **\_\_namedParameters.publicKeys**: `object`[]

#### Returns

[`MultiSigPublicKey`](MultiSigPublicKey.md)

---

### equals()

> **equals**(`publicKey`): `boolean`

Checks if two MultiSig public keys are equal

#### Parameters

• **publicKey**: [`MultiSigPublicKey`](MultiSigPublicKey.md)

#### Returns

`boolean`

#### Overrides

[`PublicKey`](../../cryptography/classes/PublicKey.md).[`equals`](../../cryptography/classes/PublicKey.md#equals)

---

### toRawBytes()

> **toRawBytes**(): `Uint8Array`

Return the byte array representation of the MultiSig public key

#### Returns

`Uint8Array`

#### Overrides

[`PublicKey`](../../cryptography/classes/PublicKey.md).[`toRawBytes`](../../cryptography/classes/PublicKey.md#torawbytes)

---

### getPublicKeys()

> **getPublicKeys**(): `object`[]

#### Returns

`object`[]

---

### getThreshold()

> **getThreshold**(): `number`

#### Returns

`number`

---

### getSigner()

> **getSigner**(...`signers`): [`MultiSigSigner`](MultiSigSigner.md)

#### Parameters

• ...**signers**: [[`Signer`](../../cryptography/classes/Signer.md)]

#### Returns

[`MultiSigSigner`](MultiSigSigner.md)

---

### toIotaAddress()

> **toIotaAddress**(): `string`

Return the IOTA address associated with this MultiSig public key

#### Returns

`string`

#### Overrides

[`PublicKey`](../../cryptography/classes/PublicKey.md).[`toIotaAddress`](../../cryptography/classes/PublicKey.md#toiotaaddress)

---

### flag()

> **flag**(): `number`

Return the IOTA address associated with this MultiSig public key

#### Returns

`number`

#### Overrides

[`PublicKey`](../../cryptography/classes/PublicKey.md).[`flag`](../../cryptography/classes/PublicKey.md#flag)

---

### verify()

> **verify**(`message`, `multisigSignature`): `Promise`\<`boolean`\>

Verifies that the signature is valid for for the provided message

#### Parameters

• **message**: `Uint8Array`

• **multisigSignature**: `string`

#### Returns

`Promise`\<`boolean`\>

#### Overrides

[`PublicKey`](../../cryptography/classes/PublicKey.md).[`verify`](../../cryptography/classes/PublicKey.md#verify)

---

### combinePartialSignatures()

> **combinePartialSignatures**(`signatures`): `string`

Combines multiple partial signatures into a single multisig, ensuring that each public key signs only once
and that all the public keys involved are known and valid, and then serializes multisig into the standard format

#### Parameters

• **signatures**: `string`[]

#### Returns

`string`
