import { IotaClient } from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';

const client = new IotaClient({ url: 'https://example.com' });
const keypair = new Ed25519Keypair();

const tx = new Transaction();

const result = await client.signAndExecuteTransaction({
    transaction: tx,
    signer: keypair,
    requestType: 'WaitForLocalExecution',
    options: {
        showEffects: true,
    },
});
