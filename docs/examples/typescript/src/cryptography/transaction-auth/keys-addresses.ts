import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';

const TEST_MNEMONIC =
    'film crazy soon outside stand loop subway crumble thrive popular green nuclear struggle pistol arm wife phrase warfare march wheat nephew ask sunny firm';

const keypair = Ed25519Keypair.deriveKeypair(TEST_MNEMONIC, `m/44'/4218'/0'/0'/0'`);
const address = keypair.getPublicKey().toIotaAddress();
