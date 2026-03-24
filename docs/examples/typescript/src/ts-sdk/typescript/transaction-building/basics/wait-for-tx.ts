import { Transaction } from '@iota/iota-sdk/transactions';
import { IotaClient } from '@iota/iota-sdk/client';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';

const client = new IotaClient({ url: 'https://example.com' });
const keypair = new Ed25519Keypair();

const tx = new Transaction();

const result = await client.signAndExecuteTransaction({ signer: keypair, transaction: tx });
await client.waitForTransaction({ digest: result.digest });
