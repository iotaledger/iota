// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { blake2b } from '@noble/hashes/blake2b';
import { describe, expect, it } from 'vitest';

import { messageWithIntent, parseSerializedSignature } from '../../src/cryptography';
import { Ed25519Keypair } from '../../src/keypairs/ed25519';
import { Secp256k1Keypair } from '../../src/keypairs/secp256k1';
import { fromB64, toB64 } from '../../src/utils';

const TX_BYTES =
    'AAACAQDMdYtdFSLGe6VbgpuIsMksv9Ypzpvkq2jiYq0hAjUpOQIAAAAAAAAAIHGwPza+lUm6RuJV1vn9pA4y0PwVT7k/KMMbUViQS5ydACAMVn/9+BYsttUa90vgGZRDuS6CPUumztJN5cbEY3l9RgEBAQEAAAEBAHUFfdk1Tg9l6STLBoSBJbbUuehTDUlLH7p81kpqCKsaBCiJ034Ac84f1oqgmpz79O8L/UeLNDUpOUMa+LadeX93AgAAAAAAAAAgs1e67e789jSlrzOJUXq0bb7Bn/hji+3F5UoMAbze595xCSZCVjU1ItUC9G7KQjygNiBbzZe8t7YLPjRAQyGTzAIAAAAAAAAAIAujHFcrkJJhZfCmxmCHsBWxj5xkviUqB479oupdgMZu07b+hkrjyvCcX50dO30v3PszXFj7+lCNTUTuE4UI3eoCAAAAAAAAACBIv39dyVELUFTkNv72mat5R1uHFkQdViikc1lTMiSVlOD+eESUq3neyciBatafk9dHuhhrS37RaSflqKwFlwzPAgAAAAAAAAAg8gqL3hCkAho8bb0PoqshJdqQFoRP8ZmQMZDFvsGBqa11BX3ZNU4PZekkywaEgSW21LnoUw1JSx+6fNZKagirGgEAAAAAAAAAKgQAAAAAAAAA';
const DIGEST = 'VMVv+/L/EG7/yhEbCQ1qiSt30JXV8yIm+4xO6yTkqeM=';
const DERIVATION_PATH = `m/44'/4218'/0'/0'/0'`;
const DERIVATION_PATH_SECP256K1 = `m/54'/4218'/0'/0/0`;

// Test cases for Ed25519.
// First element is the mnemonics, second element is the
// base64 encoded pubkey, derived using DERIVATION_PATH,
// third element is the hex encoded address, fourth
// element is the valid signature produced for TX_BYTES.
const TEST_CASES = [
    [
        'film crazy soon outside stand loop subway crumble thrive popular green nuclear struggle pistol arm wife phrase warfare march wheat nephew ask sunny firm',
        'e2BQLLmX+Ykl1g8bpJIEyon3f7Urhjlqk0Lz1BRmwho=',
        '0x9f8e5379678525edf768d7b507dc1ba9016fc4f0eac976ab7f74077d95fba312',
        'G6mBG9YpJjopAG0XvJE7CPXmWw9Rx1RFzbi7BOaOwh0OckXC3eoHC40kWGVS0sMSPnw8sed6aAEnWnZ5EXx3Ag==',
    ],
    [
        'require decline left thought grid priority false tiny gasp angle royal system attack beef setup reward aunt skill wasp tray vital bounce inflict level',
        'DyWpaP6u6fghsuEG/nR75Zc8pL9dCslo9y84FmCE5TQ=',
        '0x862738192e40540e0a5c9a5aca636f53b0cd76b0a9bef3386e05647feb4914ac',
        'MlTruVTOFz1GXQzSkIWoNmkzXrIRQxp1vKxuSFFfQacxQz6sqZR+PpRifHwqp6n3KMgutq/Q0J0PWhI2n6LSAA==',
    ],
    [
        'organ crash swim stick traffic remember army arctic mesh slice swear summer police vast chaos cradle squirrel hood useless evidence pet hub soap lake',
        '3kA3AThQsJsxb7WWx45yOZ0DUJzf+/BckV0gKlgsUmo=',
        '0x2391788ca49c7f0f00699bc2bad45f80c343b4d1df024285c132259433d7ff31',
        'UJEPE2EIXvcsRwpP3MM1DlOsUjDBmrfykn9Ztewsr78UE78ZhuqUHtbeSrLTdnCyMgHipSKwxL5PEYz654DJDw==',
    ],
];

// Test cases for Secp256k1.
// First element is the mnemonics, second element is the
// base64 encoded pubkey, derived using DERIVATION_PATH_SECP256K1,
// third element is the hex encoded address, fourth
// element is the valid signature produced for TX_BYTES.
const TEST_CASES_SECP256K1 = [
    [
        'film crazy soon outside stand loop subway crumble thrive popular green nuclear struggle pistol arm wife phrase warfare march wheat nephew ask sunny firm',
        'AsLKpMJAxwoRo5T4JCdKAVCdCH7dvfF7yQxLi9etcxR4',
        '0x8520d58dde1ab268349b9a46e5124ae6fe7e4c61df4ca2bc9c97d3c4d07b0b55',
        'sQqwpFAP9KXUrQDL/ltD3xXfNce16TaWwP83O4r5CD4XAPJgGO/lrUw+I7ec7FIgk3PSyktcALehBaBl95mgfg==',
    ],
    [
        'require decline left thought grid priority false tiny gasp angle royal system attack beef setup reward aunt skill wasp tray vital bounce inflict level',
        'ApoKn4lYZqNSwz1P8bAjGDiduZfaW3pKPp/IwJ/neSBU',
        '0x3740d570eefba29dfc0fdd5829848902064e31ecd059ca05c401907fa8646f61',
        'idUbzBef+CDWLX+y8yzk2Pn6UnV7xwvypQfJ74TzcYpz4bSFE/t7I90Gq5pGAagxYCZYhOModfezUvgH35cwXQ==',
    ],
    [
        'organ crash swim stick traffic remember army arctic mesh slice swear summer police vast chaos cradle squirrel hood useless evidence pet hub soap lake',
        'AlZDfLhVwZpYuc8ZkDhwJpWmq57E5Kswk4Ph/erI6VEN',
        '0x943b852c37fef403047e06ff5a2fa216557a4386212fb29554babdd3e1899da5',
        'A11EgmPIeNGcD+XpNh87wgD7MObrC4HX56kpC93TVJZk76c6Bijm7E4D6JM1iyzDIECOqaLDbcmBnoAQTEutxA==',
    ],
];

describe('Keypairs', () => {
    it('Ed25519 keypair signData', async () => {
        const tx_bytes = fromB64(TX_BYTES);
        const intentMessage = messageWithIntent('TransactionData', tx_bytes);

        const digest = blake2b(intentMessage, { dkLen: 32 });
        expect(toB64(digest)).toEqual(DIGEST);

        for (const t of TEST_CASES) {
            const keypair = Ed25519Keypair.deriveKeypair(t[0], DERIVATION_PATH);
            expect(keypair.getPublicKey().toBase64()).toEqual(t[1]);
            expect(keypair.getPublicKey().toIotaAddress()).toEqual(t[2]);

            const { signature: serializedSignature } = await keypair.signTransaction(tx_bytes);
            const { signature } = parseSerializedSignature(serializedSignature);

            expect(toB64(signature!)).toEqual(t[3]);

            const isValid = await keypair
                .getPublicKey()
                .verifyTransaction(tx_bytes, serializedSignature);
            expect(isValid).toBeTruthy();
        }
    });

    it('Ed25519 keypair signMessage', async () => {
        const keypair = new Ed25519Keypair();
        const signData = new TextEncoder().encode('hello world');

        const { signature } = await keypair.signPersonalMessage(signData);
        const isValid = await keypair.getPublicKey().verifyPersonalMessage(signData, signature);
        expect(isValid).toBe(true);
    });

    it('Ed25519 keypair invalid signMessage', async () => {
        const keypair = new Ed25519Keypair();
        const signData = new TextEncoder().encode('hello world');

        const { signature } = await keypair.signPersonalMessage(signData);
        const isValid = await keypair
            .getPublicKey()
            .verifyPersonalMessage(new TextEncoder().encode('hello worlds'), signature);
        expect(isValid).toBe(false);
    });

    it('Secp256k1 keypair signData', async () => {
        const tx_bytes = fromB64(TX_BYTES);
        const intentMessage = messageWithIntent('TransactionData', tx_bytes);
        const digest = blake2b(intentMessage, { dkLen: 32 });
        expect(toB64(digest)).toEqual(DIGEST);

        for (const t of TEST_CASES_SECP256K1) {
            const keypair = Secp256k1Keypair.deriveKeypair(t[0], DERIVATION_PATH_SECP256K1);
            expect(keypair.getPublicKey().toBase64()).toEqual(t[1]);
            expect(keypair.getPublicKey().toIotaAddress()).toEqual(t[2]);

            const { signature: serializedSignature } = await keypair.signTransaction(tx_bytes);
            const { signature } = parseSerializedSignature(serializedSignature);

            expect(toB64(signature!)).toEqual(t[3]);

            const isValid = await keypair
                .getPublicKey()
                .verifyTransaction(tx_bytes, serializedSignature);
            expect(isValid).toBeTruthy();
        }
    });

    it('Secp256k1 keypair signMessage', async () => {
        const keypair = new Secp256k1Keypair();
        const signData = new TextEncoder().encode('hello world');

        const { signature } = await keypair.signPersonalMessage(signData);

        const isValid = await keypair.getPublicKey().verifyPersonalMessage(signData, signature);
        expect(isValid).toBe(true);
    });
});
