import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';

const exampleMnemonic = 'result crisp session latin ...';

const keyPair = Ed25519Keypair.deriveKeypair(exampleMnemonic);
