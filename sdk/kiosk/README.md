# Kiosk SDK

Kiosk SDK is part of the **IOTA Rebased SDK**, designed specifically for interacting with the IOTA Rebased protocol. 

> **Note**: This technology is currently available **only on Testnet and Devnet**, and is **not yet supported on Mainnet**.

This Kiosk SDK library provides different utilities to interact/create/manage a
[Kiosk](https://github.com/iotaledger/iota/tree/develop/kiosk).

[You can read the documentation and see examples by clicking here.](https://docs.iota.org/references/ts-sdk/kiosk)

## Install from NPM

To use the Kiosk SDK in your project, run the following command in your project root:

```sh npm2yarn
npm i @iota/kiosk @iota/iota-sdk
```

To use the Kiosk SDK, you must create a [KioskClient](https://docs.iota.org/references/ts-sdk/kiosk/kiosk-client/introduction) instance.

## Creating a kiosk client

You can follow the example to create a KioskClient. The client currently supports MAINNET and TESTNET. View next section for usage in other networks.

```
import { KioskClient, Network } from '@iota/kiosk';
import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';

// We need a IOTA Client. You can re-use the IotaClient of your project
// (it's not recommended to create a new one).
const client = new IotaClient({ url: getFullnodeUrl('testnet') });

// Now we can use it to create a kiosk Client.
const kioskClient = new KioskClient({
    client,
    network: Network.TESTNET,
});
```

You can read the kioskClient documentation to query kiosk data [here](https://docs.iota.org/references/ts-sdk/kiosk/kiosk-client/querying).