[Ledger Github](https://github.com/LedgerHQ/ledgerjs/),
[Ledger Developer Portal](https://developers.ledger.com/),
[Ledger Developer Discord](https://developers.ledger.com/discord-pro)

# ledgerjs-hw-app-iota

`@iota/ledgerjs-hw-app-iota` is part of the **IOTA Rebased SDK**, designed specifically for interacting with the IOTA Rebased protocol.

[Ledger Hardware Wallet](https://www.ledger.com/) JavaScript bindings for [IOTA](https://iota.org/),
based on [LedgerJS](https://github.com/LedgerHQ/ledgerjs).

## Using LedgerJS for IOTA

Here is a sample app for Node:

```javascript
const Transport = require('@ledgerhq/hw-transport').default;
const Iota = require('@iota/ledgerjs-hw-app-iota').default;

const getPublicKey = async () => {
    const iota = new Iota(await Transport.create());
    return await iota.getPublicKey("44'/4218'/0'/0'/0'");
};

const signTransaction = async () => {
    const iota = new Iota(await Transport.create());
    return await iota.signTransaction("44'/4218'/0'/0'/0'", '<transaction contents>');
};

const getVersion = async () => {
    const iota = new Iota(await Transport.create());
    return await iota.getVersion();
};

const doAll = async () => {
    console.log(await getPublicKey());
    console.log(await signTransaction());
    console.log(await getVersion());
};

doAll().catch((err) => console.log(err));
```

## API

### Table of Contents

- [IOTA](#iota)
  - [Parameters](#parameters)
  - [Examples](#examples)
  - [getPublicKey](#getpublickey)
    - [Parameters](#parameters-1)
    - [Examples](#examples-1)
  - [signTransaction](#signtransaction)
    - [Parameters](#parameters-2)
    - [Examples](#examples-2)
  - [getVersion](#signtransaction)
    - [Parameters](#parameters-3)
    - [Examples](#examples-3)

### Parameters

- `transport` **`Transport<any>`**
- `scrambleKey`
  **[string](https://developer.mozilla.org/docs/Web/JavaScript/Reference/Global_Objects/String)**
  (optional, default `"Iota"`)

### Examples

```javascript
import Iota from 'ledgerjs-hw-app-iota';

const transport = await Transport.create();
const iota = new Iota(transport);
```

### getPublicKey

Gets the IOTA address for a given BIP-32 path.

#### Parameters

- `path`
  **[string](https://developer.mozilla.org/docs/Web/JavaScript/Reference/Global_Objects/String)**
  a path in BIP-32 format
- `displayOnDevice`
  **[boolean](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Boolean)**
  whether or not to display the address on the Ledger device.

#### Examples

```javascript
const publicKey = await iota.getPublicKey("44'/4218'/0'/0'/0'");
```

Returns
**[Promise](https://developer.mozilla.org/docs/Web/JavaScript/Reference/Global_Objects/Promise)&lt;[string](https://developer.mozilla.org/docs/Web/JavaScript/Reference/Global_Objects/String)>**
an object with a public key.

### signTransaction

Sign a transaction with a given BIP-32 path.

#### Parameters

- `path`
  **[string](https://developer.mozilla.org/docs/Web/JavaScript/Reference/Global_Objects/String)**
  a path in BIP-32 format

#### Examples

```javascript
const publicKey = await iota.signTransaction("44'/4218'/0'/0'/0'", '<transaction contents>');
```

Returns
**[Promise](https://developer.mozilla.org/docs/Web/JavaScript/Reference/Global_Objects/Promise)&lt;[object](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Object)>**
an object with text field containing a signature.

### getVersion

Get the version of the application installed on the hardware device.

#### Examples

```javascript
console.log(await iota.getVersion());
```

for version 0.1.0, it produces something like

```
{
  major: 0
  minor: 1
  patch: 0
}
```

Returns
**[Promise](https://developer.mozilla.org/docs/Web/JavaScript/Reference/Global_Objects/Promise)&lt;{[object](https://developer.mozilla.org/docs/Web/JavaScript/Reference/Global_Objects/Object)}>**
an object with major, minor, and patch of the version.
