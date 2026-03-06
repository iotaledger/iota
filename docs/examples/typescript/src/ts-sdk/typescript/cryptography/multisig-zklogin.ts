import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { MultiSigPublicKey } from '@iota/iota-sdk/multisig';
import { fromBase64 } from '@iota/iota-sdk/utils';
import type { PublicKey } from '@iota/iota-sdk/cryptography';

// zkLogin helpers — in a real app these come from your zkLogin integration
function decodeJwt(_token: string): { sub: string; aud: string | string[]; iss: string } {
    return { sub: 'user-subject-id', aud: 'your-app-client-id', iss: 'https://accounts.google.com' };
}
function genAddressSeed(
    _salt: bigint,
    _claimName: string,
    _claimValue: string | undefined,
    _aud: string | string[],
): bigint {
    return 0n;
}
function toZkLoginPublicIdentifier(_addressSeed: string, _iss: string): PublicKey {
    // In a real app, construct the zkLogin public identifier from the address seed and issuer
    return new Ed25519Keypair().getPublicKey();
}
function getZkLoginSignature(_opts: {
    inputs: unknown;
    maxEpoch: string;
    userSignature: Uint8Array;
}): string {
    // In a real app, call the zkLogin prover service to get the signature
    return '';
}
const zkLoginInputs = {};
const ephemeralSig = '';

const kp1 = new Ed25519Keypair();
const pkSingle = kp1.getPublicKey();

const decodedJWT = decodeJwt('a valid jwt token here');
const userSalt = BigInt('123');
const addressSeed = genAddressSeed(userSalt, 'sub', decodedJWT.sub, decodedJWT.aud).toString();

let pkZklogin = toZkLoginPublicIdentifier(addressSeed, decodedJWT.iss);

const multiSigPublicKey = MultiSigPublicKey.fromPublicKeys({
    threshold: 1,
    publicKeys: [
        { publicKey: pkSingle, weight: 1 },
        { publicKey: pkZklogin, weight: 1 },
    ],
});

const multisigAddress = multiSigPublicKey.toIotaAddress();

const zkLoginSig = getZkLoginSignature({
    inputs: zkLoginInputs,
    maxEpoch: '2',
    userSignature: fromBase64(ephemeralSig),
});

const multisig = multiSigPublicKey.combinePartialSignatures([zkLoginSig]);
