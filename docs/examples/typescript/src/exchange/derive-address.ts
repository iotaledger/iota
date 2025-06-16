import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';

const keypair = new Ed25519Keypair();
const address = keypair.getPublicKey().toIotaAddress();
console.log(address);
