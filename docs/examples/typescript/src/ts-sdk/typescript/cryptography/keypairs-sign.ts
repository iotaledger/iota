import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';

const keypair = new Ed25519Keypair();
const publicKey = keypair.getPublicKey();
const message = new TextEncoder().encode('hello world');

const { signature } = await keypair.signPersonalMessage(message);
const isValid = await publicKey.verifyPersonalMessage(message, signature);
