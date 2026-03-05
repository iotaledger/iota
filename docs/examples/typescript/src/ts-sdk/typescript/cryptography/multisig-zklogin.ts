import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { MultiSigPublicKey } from '@iota/iota-sdk/multisig';
import { fromBase64 } from '@iota/iota-sdk/utils';

// zklogin stubs (not yet available in this SDK version)
declare function decodeJwt(token: string): { sub: string; aud: string | string[]; iss: string };
declare function genAddressSeed(salt: bigint, claimName: string, claimValue: string | undefined, aud: string | string[]): bigint;
declare function toZkLoginPublicIdentifier(addressSeed: string, iss: string): import('@iota/iota-sdk/cryptography').PublicKey;
declare function getZkLoginSignature(opts: { inputs: unknown; maxEpoch: string; userSignature: Uint8Array }): string;
declare const zkLoginInputs: unknown;
declare const ephemeralSig: string;

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
