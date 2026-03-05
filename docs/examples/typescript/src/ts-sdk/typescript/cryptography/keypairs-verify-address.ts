import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { verifyPersonalMessageSignature } from '@iota/iota-sdk/verify';

const keypair = new Ed25519Keypair();
const message = new TextEncoder().encode('hello world');
const { signature } = await keypair.signPersonalMessage(message);

await verifyPersonalMessageSignature(message, signature, {
	address: keypair.getPublicKey().toIotaAddress(),
});
