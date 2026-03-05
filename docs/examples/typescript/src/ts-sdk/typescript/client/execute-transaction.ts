import { IotaClient } from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';

const client = new IotaClient({ url: 'https://example.com' });
const keypair = new Ed25519Keypair();

const tx = new Transaction();

const { bytes, signature } = await tx.sign({ client, signer: keypair });

const result = await client.executeTransactionBlock({
    transactionBlock: bytes,
    signature,
    requestType: 'WaitForLocalExecution',
    options: {
        showEffects: true,
    },
});
