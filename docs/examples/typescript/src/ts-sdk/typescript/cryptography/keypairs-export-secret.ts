import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
declare const keypair: Ed25519Keypair;

const secretKey = keypair.getSecretKey();
