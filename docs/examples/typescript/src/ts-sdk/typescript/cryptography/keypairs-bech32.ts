import { decodeIotaPrivateKey, encodeIotaPrivateKey } from '@iota/iota-sdk/cryptography';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';

const encoded = encodeIotaPrivateKey(
    Uint8Array.from([
        59, 148, 11, 85, 134, 130, 61, 253, 2, 174, 59, 70, 27, 180, 51, 107, 94, 203, 174, 253, 102,
        39, 170, 146, 46, 252, 4, 143, 236, 12, 136, 28,
    ]),
    'ED25519',
);
const { schema, secretKey } = decodeIotaPrivateKey(encoded);
const keypair = Ed25519Keypair.fromSecretKey(secretKey);
