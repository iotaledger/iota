# IOTA Signers

The IOTA Signers package provides a set of tools for securely signing transactions with i.e. hardware wallets.

## Table of Contents

- [IOTA Signers](#iota-signers)
  - [Table of Contents](#table-of-contents)
  - [Ledger Signer](#ledger-signer)
    - [Usage](#usage-2)
      - [fromDerivationPath](#fromderivationpath)
        - [Parameters](#parameters-2)
        - [Examples](#examples-2)

## Ledger Signer

The Ledger Signer allows you to leverage a Ledger hardware wallet to sign IOTA transactions.

### Usage

#### fromDerivationPath

Creates a Ledger signer from the provided options. This method initializes the signer with the
necessary configuration, allowing it to interact with a Ledger hardware wallet to perform
cryptographic operations.

##### Parameters

- `options`
  **[object](https://developer.mozilla.org/docs/Web/JavaScript/Reference/Global_Objects/Object)** An
  object containing GCP credentials and configuration.
  - `projectId`
    **[string](https://developer.mozilla.org/docs/Web/JavaScript/Reference/Global_Objects/String)**
    The GCP project ID.

##### Examples

```typescript
import Transport from '@ledgerhq/hw-transport-node-hid';
import IotaLedgerClient from '@iota/ledgerjs-hw-app-iota';
import { LedgerSigner } from '@iota/signers/ledger';
import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';

const transport = await Transport.open(undefined);
const ledgerClient = new IotaLedgerClient(transport);
const iotaClient = new IotaClient({ url: getFullnodeUrl('testnet') });

const signer = await LedgerSigner.fromDerivationPath(
	"m/44'/4218'/0'/0'/0'",
	ledgerClient,
	iotaClient,
);

// Log the IOTA address:
console.log(signer.toIOTAAddress());

// Define a test transaction:
const testTransaction = new Transaction();
const transactionBytes = await testTransaction.build();

// Sign a test transaction:
const { signature } = await signer.signTransaction(transactionBytes);
console.log(signature);
```
