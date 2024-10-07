// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { fromB64, toB58 } from '@iota/bcs';
import nacl from 'tweetnacl';
import { describe, expect, it } from 'vitest';

import { decodeIotaPrivateKey } from '../../../src/cryptography/keypair';
import { Ed25519Keypair } from '../../../src/keypairs/ed25519';
import { Transaction } from '../../../src/transactions';
import { verifyPersonalMessageSignature, verifyTransactionSignature } from '../../../src/verify';

const VALID_SECRET_KEY = 'mdqVWeFekT7pqy5T49+tV12jO0m+ESW7ki4zSU9JiCg=';
const PRIVATE_KEY_SIZE = 32;

// Test case generated against rust keytool cli. See https://github.com/iotaledger/iota/blob/edd2cd31e0b05d336b1b03b6e79a67d8dd00d06b/crates/iota/src/unit_tests/keytool_tests.rs#L165
const TEST_CASES = [
    [
        'film crazy soon outside stand loop subway crumble thrive popular green nuclear struggle pistol arm wife phrase warfare march wheat nephew ask sunny firm',
        'iotaprivkey1qrqqxhsu3ndp96644fjk4z5ams5ulgmvprklngt2jhvg2ujn5w4q2d2vplv',
        '0x9f8e5379678525edf768d7b507dc1ba9016fc4f0eac976ab7f74077d95fba312',
    ],
    [
        'require decline left thought grid priority false tiny gasp angle royal system attack beef setup reward aunt skill wasp tray vital bounce inflict level',
        'iotaprivkey1qqcxaf57fnenvflpacacaumf6vl0rt0edddhytanvzhkqhwnjk0zspg902d',
        '0x862738192e40540e0a5c9a5aca636f53b0cd76b0a9bef3386e05647feb4914ac',
    ],
    [
        'organ crash swim stick traffic remember army arctic mesh slice swear summer police vast chaos cradle squirrel hood useless evidence pet hub soap lake',
        'iotaprivkey1qzq39vxzm0gq7l8dc5dj5allpuww4mavhwhg8mua4cl3lj2c3fvhcv5l2vn',
        '0x2391788ca49c7f0f00699bc2bad45f80c343b4d1df024285c132259433d7ff31',
    ],
];

describe('ed25519-keypair', () => {
    it('new keypair', () => {
        const keypair = new Ed25519Keypair();
        expect(keypair.getPublicKey().toRawBytes().length).toBe(32);
        expect(2).toEqual(2);
    });

    it('create keypair from secret key', () => {
        const secretKey = fromB64(VALID_SECRET_KEY);
        const keypair = Ed25519Keypair.fromSecretKey(secretKey);
        expect(keypair.getPublicKey().toBase64()).toEqual(
            'Gy9JCW4+Xb0Pz6nAwM2S2as7IVRLNNXdSmXZi4eLmSI=',
        );
    });

    it('create keypair from secret key and mnemonics matches keytool', () => {
        for (const t of TEST_CASES) {
            // Keypair derived from mnemonic.
            const keypair = Ed25519Keypair.deriveKeypair(t[0]);
            expect(keypair.getPublicKey().toIotaAddress()).toEqual(t[2]);

            // Decode Iota private key from Bech32 string
            const parsed = decodeIotaPrivateKey(t[1]);
            const kp = Ed25519Keypair.fromSecretKey(parsed.secretKey);
            expect(kp.getPublicKey().toIotaAddress()).toEqual(t[2]);

            // Exported keypair matches the Bech32 encoded secret key.
            const exported = kp.getSecretKey();
            expect(exported).toEqual(t[1]);
        }
    });

    it('generate keypair from random seed', () => {
        const keypair = Ed25519Keypair.fromSecretKey(
            Uint8Array.from(Array(PRIVATE_KEY_SIZE).fill(8)),
        );
        expect(keypair.getPublicKey().toBase64()).toEqual(
            'E5j2LG0aRXxRumpLXz29L2n8qTIWIY3ImX5Ba9F9k8o=',
        );
    });

    it('signature of data is valid', async () => {
        const keypair = new Ed25519Keypair();
        const signData = new TextEncoder().encode('hello world');
        const signature = await keypair.sign(signData);
        const isValid = nacl.sign.detached.verify(
            signData,
            signature,
            keypair.getPublicKey().toRawBytes(),
        );
        expect(isValid).toBeTruthy();
        expect(keypair.getPublicKey().verify(signData, signature));
    });

    it('incorrect coin type node for ed25519 derivation path', async () => {
        const keypair = Ed25519Keypair.deriveKeypair(TEST_CASES[0][0], `m/44'/4218'/0'/0'/0'`);

        const signData = new TextEncoder().encode('hello world');
        const signature = await keypair.sign(signData);
        const isValid = nacl.sign.detached.verify(
            signData,
            signature,
            keypair.getPublicKey().toRawBytes(),
        );
        expect(isValid).toBeTruthy();
    });

    it('incorrect coin type node for ed25519 derivation path', () => {
        expect(() => {
            Ed25519Keypair.deriveKeypair(TEST_CASES[0][0], `m/44'/0'/0'/0'/0'`);
        }).toThrow('Invalid derivation path');
    });

    it('incorrect purpose node for ed25519 derivation path', () => {
        expect(() => {
            Ed25519Keypair.deriveKeypair(TEST_CASES[0][0], `m/54'/4218'/0'/0'/0'`);
        }).toThrow('Invalid derivation path');
    });

    it('invalid mnemonics to derive ed25519 keypair', () => {
        expect(() => {
            Ed25519Keypair.deriveKeypair('aaa');
        }).toThrow('Invalid mnemonic');
    });

    it('signs Transactions', async () => {
        const keypair = new Ed25519Keypair();
        const tx = new Transaction();
        tx.setSender(keypair.getPublicKey().toIotaAddress());
        tx.setGasPrice(5);
        tx.setGasBudget(100);
        tx.setGasPayment([
            {
                objectId: (Math.random() * 100000).toFixed(0).padEnd(64, '0'),
                version: String((Math.random() * 10000).toFixed(0)),
                digest: toB58(
                    new Uint8Array([
                        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4,
                        5, 6, 7, 8, 9, 1, 2,
                    ]),
                ),
            },
        ]);

        const bytes = await tx.build();

        const serializedSignature = (await keypair.signTransaction(bytes)).signature;

        expect(await keypair.getPublicKey().verifyTransaction(bytes, serializedSignature)).toEqual(
            true,
        );
        expect(await keypair.getPublicKey().verifyTransaction(bytes, serializedSignature)).toEqual(
            true,
        );
        expect(!!(await verifyTransactionSignature(bytes, serializedSignature))).toEqual(true);
    });

    it('signs PersonalMessages', async () => {
        const keypair = new Ed25519Keypair();
        const message = new TextEncoder().encode('hello world');

        const serializedSignature = (await keypair.signPersonalMessage(message)).signature;

        expect(
            await keypair.getPublicKey().verifyPersonalMessage(message, serializedSignature),
        ).toEqual(true);
        expect(
            await keypair.getPublicKey().verifyPersonalMessage(message, serializedSignature),
        ).toEqual(true);
        expect(!!(await verifyPersonalMessageSignature(message, serializedSignature))).toEqual(
            true,
        );
    });
});
