import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';

declare const TEST_MNEMONIC: string;

const keypair = Ed25519Keypair.deriveKeypair(TEST_MNEMONIC, `m/44'/4218'/0'/0'/0'`);
const address = keypair.getPublicKey().toIotaAddress();
