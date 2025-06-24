import {
    AccountsContractMethod,
    CoreContract,
    getHname,
    IscTransaction,
    L2_FROM_L1_GAS_BUDGET,
} from '../src/index';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { requestIotaFromFaucetV0 } from '@iota/iota-sdk/faucet';
import { IotaClient } from '@iota/iota-sdk/client';
import { CONFIG } from './config';

const { L1 } = CONFIG;

const client = new IotaClient({
    url: L1.rpcUrl,
});

const keypair = new Ed25519Keypair();
const address = keypair.toIotaAddress();

console.log('Requesting faucet...');

await requestIotaFromFaucetV0({
    host: L1.faucetUrl,
    recipient: address,
});

console.log('Sending...');

// EVM Address
const recipientAddress = process.argv[2];
// Amount to send (1 IOTAs)
const amountToSend = BigInt(1 * 1000000000);
// We also need to place a little more in the bag to cover the L2 gas
const amountToPlace = amountToSend + L2_FROM_L1_GAS_BUDGET;

const iscTx = new IscTransaction(L1);

const bag = iscTx.newBag();
const coin = iscTx.coinFromAmount({ amount: amountToPlace });
iscTx.placeCoinInBag({ coin, bag });
iscTx.createAndSendToEvm({
    bag,
    transfers: [[IOTA_TYPE_ARG, amountToSend]],
    address: recipientAddress,
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
