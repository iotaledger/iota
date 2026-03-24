import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { verifyTransactionSignature } from '@iota/iota-sdk/verify';
import { Transaction } from '@iota/iota-sdk/transactions';

const client = new IotaClient({ url: getFullnodeUrl('testnet') });
const tx = new Transaction();
const bytes = await tx.build({ client });

const keypair = new Ed25519Keypair();
const { signature } = await keypair.signTransaction(bytes);

await verifyTransactionSignature(bytes, signature, {
	address: keypair.getPublicKey().toIotaAddress(),
});
