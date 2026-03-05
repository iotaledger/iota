import { decodeIotaPrivateKey } from '@iota/iota-sdk/cryptography';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';

const secretKey = 'iotaprivkey1qzse89atw7d3zum8ujep76d2cxmgduyuast0y9fu23xcl0mpafgkktllhyc';

const parsedKeypair = decodeIotaPrivateKey(secretKey);

const keypair = Ed25519Keypair.fromSecretKey(parsedKeypair.secretKey);
