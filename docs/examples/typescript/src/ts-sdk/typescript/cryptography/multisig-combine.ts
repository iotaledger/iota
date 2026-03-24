import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { MultiSigPublicKey } from '@iota/iota-sdk/multisig';

const kp1 = new Ed25519Keypair();
const kp2 = new Ed25519Keypair();

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
    ],
});

const message = new TextEncoder().encode('hello world');

const signature1 = (await kp1.signPersonalMessage(message)).signature;
const signature2 = (await kp2.signPersonalMessage(message)).signature;

const combinedSignature = multiSigPublicKey.combinePartialSignatures([signature1, signature2]);

const isValid = await multiSigPublicKey.verifyPersonalMessage(message, combinedSignature);
