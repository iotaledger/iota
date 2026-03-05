import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { fromHex } from '@iota/iota-sdk/utils';

const secret = '0x...';
const keypair = Ed25519Keypair.fromSecretKey(fromHex(secret));
