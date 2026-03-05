import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';

declare const secretKey: Uint8Array;

const keypair1 = new Ed25519Keypair();
const keypair2 = Ed25519Keypair.fromSecretKey(secretKey);
