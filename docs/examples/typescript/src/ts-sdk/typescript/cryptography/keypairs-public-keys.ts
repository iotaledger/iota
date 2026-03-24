import { Ed25519Keypair, Ed25519PublicKey } from '@iota/iota-sdk/keypairs/ed25519';

const keypair = new Ed25519Keypair();

const bytes = keypair.getPublicKey().toRawBytes();
const publicKey = new Ed25519PublicKey(bytes);
const address1 = publicKey.toIotaAddress();

const address2 = keypair.getPublicKey().toIotaAddress();
