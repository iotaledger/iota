import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { MultiSigPublicKey } from '@iota/iota-sdk/multisig';

const kp1 = new Ed25519Keypair();
const kp2 = new Ed25519Keypair();
const kp3 = new Ed25519Keypair();

const multiSigPublicKey = MultiSigPublicKey.fromPublicKeys({
    threshold: 2,
    publicKeys: [
        {
            publicKey: kp1.getPublicKey(),
            weight: 1,
        },
        {
            publicKey: kp2.getPublicKey(),
            weight: 1,
        },
        {
            publicKey: kp3.getPublicKey(),
            weight: 2,
        },
    ],
});

const multisigAddress = multiSigPublicKey.toIotaAddress();
