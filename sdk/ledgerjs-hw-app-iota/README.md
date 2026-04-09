[Ledger Github](https://github.com/LedgerHQ/ledgerjs/),
[Ledger Developer Portal](https://developers.ledger.com/),
[Ledger Developer Discord](https://developers.ledger.com/discord-pro)

# ledgerjs-hw-app-iota

`@iota/ledgerjs-hw-app-iota` is part of the **IOTA Rebased SDK**, designed specifically for interacting with the IOTA Rebased protocol.

[Ledger Hardware Wallet](https://www.ledger.com/) JavaScript bindings for [IOTA](https://iota.org/),
based on [LedgerJS](https://github.com/LedgerHQ/ledger-live).

## Example

Here is a sample app for Node:

```javascript
import Transport from '@ledgerhq/hw-transport';
import IotaLedgerClient from '@iota/ledgerjs-hw-app-iota';

const getPublicKey = async () => {
    const iota = new IotaLedgerClient(await Transport.create());
    return await iota.getPublicKey("44'/4218'/0'/0'/0'");
};

const signTransaction = async () => {
    const iota = new IotaLedgerClient(await Transport.create());
    return await iota.signTransaction("44'/4218'/0'/0'/0'", '<transaction contents>');
};

const getVersion = async () => {
    const iota = new IotaLedgerClient(await Transport.create());
    return await iota.getVersion();
};

console.log(await getPublicKey());
console.log(await signTransaction());
console.log(await getVersion());
```
