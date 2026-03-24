import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { SerialTransactionExecutor, Transaction } from '@iota/iota-sdk/transactions';

const client = new IotaClient({ url: getFullnodeUrl('devnet') });

// In a real app, load keypair from a secure source
const yourKeyPair = new Ed25519Keypair();
const address1 = yourKeyPair.getPublicKey().toIotaAddress();
const address2 = yourKeyPair.getPublicKey().toIotaAddress();

const executor = new SerialTransactionExecutor({
    client,
    signer: yourKeyPair,
});

const tx1 = new Transaction();
const [coin1] = tx1.splitCoins(tx1.gas, [1]);
tx1.transferObjects([coin1], address1);
const tx2 = new Transaction();
const [coin2] = tx2.splitCoins(tx2.gas, [1]);
tx2.transferObjects([coin2], address2);

const [{ digest: digest1 }, { digest: digest2 }] = await Promise.all([
    executor.executeTransaction(tx1),
    executor.executeTransaction(tx2),
]);

console.log({ digest1, digest2 });
