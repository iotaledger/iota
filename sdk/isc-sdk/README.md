# isc-sdk

Use the `isc-sdk` to construct IOTA transactions that call
[ISC smart contracts](https://docs.iota.org/developer/iota-evm/references/magic-contract/ISC).

### Installation

```bash
$ npm install @iota/isc-sdk
```

### Examples

Here you can find an example on how to sent some IOTA coins from L1 to L2:

```ts
import {
    AccountsContractMethod,
    CoreContract,
    getHname,
    IscTransaction,
    L2_FROM_L1_GAS_BUDGET,
} from '@iota/isc-sdk';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { requestIotaFromFaucetV0 } from '@iota/iota-sdk/faucet';
import { IotaClient } from '@iota/iota-sdk/client';

const RPC_URL = 'https://api.testnet.iota.cafe';
const FAUCET_URL = 'https://faucet.testnet.iota.cafe';
const DESTINATION_EVM_ADDRESS = '...';
const L1_CONFIG = {
    networkName: 'testnet',
    rpcUrl: 'https://api.testnet.iota.cafe',
    faucetUrl: 'https://faucet.testnet.iota.cafe',
    chainId: '0x2f11f5ea9d3c093c9cc2e329cf92e05aa00ac052ada96c4c14a2f6869a7cbcaf',
    packageId: '0x1e6e060b87f55acc0a7632acab9cf5712ff01643f8577c9a6f99ebd1010e3f4c',
    accountsContract: '0x3c4b5e02',
    accountsTransferAllowanceTo: '0x23f4e3a1',
};

const client = new IotaClient({
    url: RPC_URL,
});

const keypair = new Ed25519Keypair();
const address = keypair.toIotaAddress();

console.log('Requesting faucet...');

await requestIotaFromFaucetV0({
    host: FAUCET_URL,
    recipient: address,
});

console.log('Sending...');

// Amount to send (1 IOTAs)
const amountToSend = BigInt(1 * 1_000_000_000);
// We also need to place a little more in the bag to cover the L2 gas
const amountToPlace = amountToSend + L2_FROM_L1_GAS_BUDGET;

const iscTx = new IscTransaction(L1_CONFIG);

const bag = iscTx.newBag();
const coin = iscTx.coinFromAmount({ amount: amountToPlace });
iscTx.placeCoinInBag({ coin, bag });
iscTx.createAndSendToEvm({
    bag,
    transfers: [[IOTA_TYPE_ARG, amountToSend]],
    address: DESTINATION_EVM_ADDRESS,
    accountsContract: getHname(CoreContract.Accounts),
    accountsFunction: getHname(AccountsContractMethod.TransferAllowanceTo),
});

const transaction = iscTx.build();
transaction.setSender(address);
await transaction.build({ client });

await client.signAndExecuteTransaction({
    signer: keypair,
    transaction,
});

console.log('Sent!');
```

You can find more examples in the [examples](examples/) folder.
