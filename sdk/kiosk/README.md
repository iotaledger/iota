# Kiosk SDK

`@iota/kiosk` is part of the **IOTA Rebased SDK**, designed specifically for interacting with the IOTA Rebased protocol.

> **Note**: This package is currently supported **only in Testnet and Devnet**, it is **not yet supported in Mainnet**.

This Kiosk SDK library provides different utilities to interact/create/manage a
[Kiosk](https://github.com/iotaledger/iota/tree/develop/kiosk).

[You can read the documentation and see examples by clicking here.](https://docs.iota.org/ts-sdk/kiosk)

## Install

To use the Kiosk SDK in your project, run the following command in your project root:

```sh npm2yarn
npm i @iota/kiosk @iota/iota-sdk
```

To use the Kiosk SDK, you must create a [KioskClient](https://docs.iota.org/ts-sdk/kiosk/kiosk-client/introduction) instance.

## Setup

You can follow this example to create a KioskClient.

```typescript
import { KioskClient } from '@iota/kiosk';
import { getFullnodeUrl, IotaClient, Network } from '@iota/iota-sdk/client';

// We need a IOTA Client. You can re-use the IotaClient of your project
// (it's not recommended to create a new one).
const client = new IotaClient({ url: getFullnodeUrl(Network.Testnet) });

// Now we can use it to create a kiosk Client.
const kioskClient = new KioskClient({
    client,
    network: Network.Testnet,
});
```

You can read the KioskClient documentation to query kiosk data [here](https://docs.iota.org/ts-sdk/kiosk/kiosk-client/querying).
