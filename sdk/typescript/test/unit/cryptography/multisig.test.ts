// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { toB58, toB64 } from '@iota/bcs';
import { beforeAll, describe, expect, it } from 'vitest';

import { bcs } from '../../../src/bcs';
import { parseSerializedSignature, SIGNATURE_SCHEME_TO_FLAG } from '../../../src/cryptography';
// import { setup, TestToolbox } from './utils/setup';
import { SignatureWithBytes } from '../../../src/cryptography/keypair';
import { PublicKey } from '../../../src/cryptography/publickey';
import { Ed25519Keypair, Ed25519PublicKey } from '../../../src/keypairs/ed25519';
import { Secp256k1Keypair } from '../../../src/keypairs/secp256k1';
import { Secp256r1Keypair } from '../../../src/keypairs/secp256r1';
import { MultiSigPublicKey } from '../../../src/multisig';
import { TransactionBlock } from '../../../src/transactions';

describe('Multisig scenarios', () => {
    it('multisig address creation and combine sigs using Secp256r1Keypair', async () => {
        const k1 = new Ed25519Keypair();
        const pk1 = k1.getPublicKey();

        const k2 = new Secp256k1Keypair();
        const pk2 = k2.getPublicKey();

        const k3 = new Secp256r1Keypair();
        const pk3 = k3.getPublicKey();

        const pubkeyWeightPairs = [
            {
                publicKey: pk1,
                weight: 1,
            },
            {
                publicKey: pk2,
                weight: 2,
            },
            {
                publicKey: pk3,
                weight: 3,
            },
        ];

        const txb = new TransactionBlock();
        txb.setSender(k3.getPublicKey().toIotaAddress());
        txb.setGasPrice(5);
        txb.setGasBudget(100);
        txb.setGasPayment([
            {
                objectId: (Math.random() * 100000).toFixed(0).padEnd(64, '0'),
                version: String((Math.random() * 10000).toFixed(0)),
                digest: toB58(
                    new Uint8Array([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
                ),
            },
        ]);
        const bytes = await txb.build();

        const { signature } = await k3.signTransactionBlock(bytes);

        const publicKey = MultiSigPublicKey.fromPublicKeys({
            threshold: 3,
            publicKeys: pubkeyWeightPairs,
        });

        const multisig = publicKey.combinePartialSignatures([signature]);

        expect(await k3.getPublicKey().verifyTransactionBlock(bytes, signature)).toEqual(true);

        const parsed = parseSerializedSignature(multisig);
        const publicKey2 = new MultiSigPublicKey(parsed.multisig!.multisig_pk);

        // multisig (sig3 weight 3 >= threshold ) verifies ok
        expect(await publicKey2.verifyTransactionBlock(bytes, multisig)).toEqual(true);
    });

    it('providing false number of signatures to combining via different methods', async () => {
        const k1 = new Ed25519Keypair();
        const pk1 = k1.getPublicKey();

        const k2 = new Secp256k1Keypair();
        const pk2 = k2.getPublicKey();

        const k3 = new Secp256r1Keypair();

        const multiSigPublicKey = MultiSigPublicKey.fromPublicKeys({
            threshold: 3,
            publicKeys: [
                { publicKey: pk1, weight: 1 },
                { publicKey: pk2, weight: 2 },
            ],
        });

        const signData = new TextEncoder().encode('hello world');
        const sig1 = await k1.signPersonalMessage(signData);
        const sig2 = await k2.signPersonalMessage(signData);
        const sig3 = await k3.signPersonalMessage(signData);

        const isValidSig1 = await k1.getPublicKey().verifyPersonalMessage(signData, sig1.signature);
        const isValidSig2 = await k2.getPublicKey().verifyPersonalMessage(signData, sig2.signature);

        expect(isValidSig1).toBe(true);
        expect(isValidSig2).toBe(true);

        // create invalid signature

        const compressedSignatures: ({ ED25519: number[] } | { Secp256r1: number[] })[] = [
            {
                ED25519: Array.from(
                    parseSerializedSignature(sig1.signature).signature!.map((x: number) =>
                        Number(x),
                    ),
                ),
            },
            {
                Secp256r1: Array.from(
                    parseSerializedSignature(sig1.signature).signature!.map((x: number) =>
                        Number(x),
                    ),
                ),
            },
        ];

        const bytes = bcs.MultiSig.serialize({
            sigs: compressedSignatures,
            bitmap: 5,
            multisig_pk: bcs.MultiSigPublicKey.parse(multiSigPublicKey.toRawBytes()),
        }).toBytes();
        const tmp = new Uint8Array(bytes.length + 1);
        tmp.set([SIGNATURE_SCHEME_TO_FLAG['MultiSig']]);
        tmp.set(bytes, 1);

        const multisig = toB64(tmp);

        expect(() =>
            multiSigPublicKey.combinePartialSignatures([sig1.signature, sig3.signature]),
        ).toThrowError(new Error('Received signature from unknown public key'));

        const parsed = parseSerializedSignature(multisig);
        const publicKey = new MultiSigPublicKey(parsed.multisig!.multisig_pk);

        await expect(publicKey.verifyPersonalMessage(signData, multisig)).rejects.toThrow(
            new Error("Cannot read properties of undefined (reading 'pubKey')"),
        );
    });

    it('providing the same signature multiple times to combining via different methods', async () => {
        const k1 = new Ed25519Keypair();
        const pk1 = k1.getPublicKey();

        const k2 = new Secp256k1Keypair();
        const pk2 = k2.getPublicKey();

        const multiSigPublicKey = MultiSigPublicKey.fromPublicKeys({
            threshold: 3,
            publicKeys: [
                { publicKey: pk1, weight: 1 },
                { publicKey: pk2, weight: 2 },
            ],
        });

        const signData = new TextEncoder().encode('hello world');
        const sig1 = await k1.signPersonalMessage(signData);
        const sig2 = await k2.signPersonalMessage(signData);

        const isValidSig1 = await k1.getPublicKey().verifyPersonalMessage(signData, sig1.signature);
        const isValidSig2 = await k2.getPublicKey().verifyPersonalMessage(signData, sig2.signature);

        expect(isValidSig1).toBe(true);
        expect(isValidSig2).toBe(true);

        // create invalid signature
        const compressedSignatures: ({ ED25519: number[] } | { Secp256r1: number[] })[] = [
            {
                ED25519: Array.from(
                    parseSerializedSignature(sig1.signature).signature!.map((x: number) =>
                        Number(x),
                    ),
                ),
            },
            {
                ED25519: Array.from(
                    parseSerializedSignature(sig1.signature).signature!.map((x: number) =>
                        Number(x),
                    ),
                ),
            },
        ];

        const bytes = bcs.MultiSig.serialize({
            sigs: compressedSignatures,
            bitmap: 1,
            multisig_pk: bcs.MultiSigPublicKey.parse(multiSigPublicKey.toRawBytes()),
        }).toBytes();
        const tmp = new Uint8Array(bytes.length + 1);
        tmp.set([SIGNATURE_SCHEME_TO_FLAG['MultiSig']]);
        tmp.set(bytes, 1);

        const multisig = toB64(tmp);

        expect(() =>
            multiSigPublicKey.combinePartialSignatures([sig2.signature, sig2.signature]),
        ).toThrowError(new Error('Received multiple signatures from the same public key'));

        const parsed = parseSerializedSignature(multisig);
        const publicKey = new MultiSigPublicKey(parsed.multisig!.multisig_pk);

        await expect(publicKey.verifyPersonalMessage(signData, multisig)).rejects.toThrow(
            new Error("Cannot read properties of undefined (reading 'pubKey')"),
        );
    });

    it('providing invalid signature', async () => {
        const k1 = new Ed25519Keypair();
        const pk1 = k1.getPublicKey();

        const k2 = new Secp256k1Keypair();
        const pk2 = k2.getPublicKey();

        const multiSigPublicKey = MultiSigPublicKey.fromPublicKeys({
            threshold: 3,
            publicKeys: [
                { publicKey: pk1, weight: 1 },
                { publicKey: pk2, weight: 2 },
            ],
        });

        const signData = new TextEncoder().encode('hello world');
        const sig1 = await k1.signPersonalMessage(signData);
        const sig2 = await k2.signPersonalMessage(signData);

        // Invalid Signature.
        const sig3: SignatureWithBytes = {
            bytes: 'd',
            signature: 'd',
        };

        const isValidSig1 = await k1.getPublicKey().verifyPersonalMessage(signData, sig1.signature);
        const isValidSig2 = await k2.getPublicKey().verifyPersonalMessage(signData, sig2.signature);

        expect(isValidSig1).toBe(true);
        expect(isValidSig2).toBe(true);

        // publickey.ts
        expect(() => multiSigPublicKey.combinePartialSignatures([sig3.signature])).toThrowError();
    });

    it('providing signatures with invalid order', async () => {
        const k1 = new Secp256r1Keypair();
        const pk1 = k1.getPublicKey();

        const k2 = new Secp256k1Keypair();
        const pk2 = k2.getPublicKey();

        const multiSigPublicKey = MultiSigPublicKey.fromPublicKeys({
            threshold: 3,
            publicKeys: [
                { publicKey: pk1, weight: 1 },
                { publicKey: pk2, weight: 2 },
            ],
        });

        const signData = new TextEncoder().encode('hello world');
        const sig1 = await k1.signPersonalMessage(signData);
        const sig2 = await k2.signPersonalMessage(signData);

        const isValidSig1 = await k1.getPublicKey().verifyPersonalMessage(signData, sig1.signature);
        const isValidSig2 = await k2.getPublicKey().verifyPersonalMessage(signData, sig2.signature);

        expect(isValidSig1).toBe(true);
        expect(isValidSig2).toBe(true);

        // publickey.ts
        const multisig = multiSigPublicKey.combinePartialSignatures([
            sig2.signature,
            sig1.signature,
        ]);

        const parsed = parseSerializedSignature(multisig);
        const publicKey = new MultiSigPublicKey(parsed.multisig!.multisig_pk);

        // Invalid order can't be verified.
        expect(await publicKey.verifyPersonalMessage(signData, multisig)).toEqual(false);
        expect(await multiSigPublicKey.verifyPersonalMessage(signData, multisig)).toEqual(false);
    });

    it('providing invalid intent scope', async () => {
        const k1 = new Secp256r1Keypair();
        const pk1 = k1.getPublicKey();

        const k2 = new Secp256k1Keypair();
        const pk2 = k2.getPublicKey();

        const multiSigPublicKey = MultiSigPublicKey.fromPublicKeys({
            threshold: 3,
            publicKeys: [
                { publicKey: pk1, weight: 1 },
                { publicKey: pk2, weight: 2 },
            ],
        });

        const signData = new TextEncoder().encode('hello world');
        const sig1 = await k1.signPersonalMessage(signData);
        const sig2 = await k2.signPersonalMessage(signData);

        const isValidSig1 = await k1.getPublicKey().verifyPersonalMessage(signData, sig1.signature);
        const isValidSig2 = await k2.getPublicKey().verifyPersonalMessage(signData, sig2.signature);

        expect(isValidSig1).toBe(true);
        expect(isValidSig2).toBe(true);

        // publickey.ts
        const multisig = multiSigPublicKey.combinePartialSignatures([
            sig1.signature,
            sig2.signature,
        ]);

        const parsed = parseSerializedSignature(multisig);
        const publicKey = new MultiSigPublicKey(parsed.multisig!.multisig_pk);

        // Invalid intentScope.
        expect(await publicKey.verifyTransactionBlock(signData, multisig)).toEqual(false);
        expect(await multiSigPublicKey.verifyTransactionBlock(signData, multisig)).toEqual(false);
    });

    it('providing empty values', async () => {
        const k1 = new Secp256r1Keypair();
        const pk1 = k1.getPublicKey();

        const k2 = new Secp256k1Keypair();
        const pk2 = k2.getPublicKey();

        const multiSigPublicKey = MultiSigPublicKey.fromPublicKeys({
            threshold: 3,
            publicKeys: [
                { publicKey: pk1, weight: 1 },
                { publicKey: pk2, weight: 2 },
            ],
        });

        const signData = new TextEncoder().encode('hello world');
        const sig1 = await k1.signPersonalMessage(signData);
        const sig2 = await k2.signPersonalMessage(signData);

        const isValidSig1 = await k1.getPublicKey().verifyPersonalMessage(signData, sig1.signature);
        const isValidSig2 = await k2.getPublicKey().verifyPersonalMessage(signData, sig2.signature);

        expect(isValidSig1).toBe(true);
        expect(isValidSig2).toBe(true);

        // Empty values.
        const multisig = multiSigPublicKey.combinePartialSignatures([]);

        const parsed = parseSerializedSignature(multisig);
        const publicKey = new MultiSigPublicKey(parsed.multisig!.multisig_pk);

        // Rejects verification.
        expect(await publicKey.verifyTransactionBlock(signData, multisig)).toEqual(false);
        expect(await multiSigPublicKey.verifyTransactionBlock(signData, multisig)).toEqual(false);
    });
});

describe('Multisig address creation:', () => {
    let k1: Ed25519Keypair,
        pk1: Ed25519PublicKey,
        k2: Secp256k1Keypair,
        pk2: PublicKey,
        k3: Secp256r1Keypair,
        pk3: PublicKey;

    beforeAll(() => {
        k1 = new Ed25519Keypair();
        pk1 = k1.getPublicKey();

        k2 = new Secp256k1Keypair();
        pk2 = k2.getPublicKey();

        k3 = new Secp256r1Keypair();
        pk3 = k3.getPublicKey();

        const secret_key_ed25519 = new Uint8Array([
            126, 57, 195, 235, 248, 196, 105, 68, 115, 164, 8, 221, 100, 250, 137, 160, 245, 43,
            220, 168, 250, 73, 119, 95, 19, 242, 100, 105, 81, 114, 86, 105,
        ]);

        Ed25519Keypair.fromSecretKey(secret_key_ed25519);
    });

    it('with unreachable threshold', async () => {
        expect(() =>
            MultiSigPublicKey.fromPublicKeys({
                threshold: 7,
                publicKeys: [
                    { publicKey: pk1, weight: 1 },
                    { publicKey: pk2, weight: 2 },
                    { publicKey: pk3, weight: 3 },
                ],
            }),
        ).toThrow(new Error('Unreachable threshold'));
    });

    it('with more public keys than limited number', async () => {
        const k4 = new Secp256r1Keypair();
        const pk4 = k4.getPublicKey();

        const k5 = new Secp256r1Keypair();
        const pk5 = k5.getPublicKey();

        const k6 = new Secp256r1Keypair();
        const pk6 = k6.getPublicKey();

        const k7 = new Secp256r1Keypair();
        const pk7 = k7.getPublicKey();

        const k8 = new Secp256r1Keypair();
        const pk8 = k8.getPublicKey();

        const k9 = new Secp256r1Keypair();
        const pk9 = k9.getPublicKey();

        const k10 = new Secp256r1Keypair();
        const pk10 = k10.getPublicKey();

        const k11 = new Secp256r1Keypair();
        const pk11 = k11.getPublicKey();

        expect(() =>
            MultiSigPublicKey.fromPublicKeys({
                threshold: 10,
                publicKeys: [
                    { publicKey: pk1, weight: 1 },
                    { publicKey: pk2, weight: 2 },
                    { publicKey: pk3, weight: 3 },
                    { publicKey: pk4, weight: 4 },
                    { publicKey: pk5, weight: 5 },
                    { publicKey: pk6, weight: 1 },
                    { publicKey: pk7, weight: 2 },
                    { publicKey: pk8, weight: 3 },
                    { publicKey: pk9, weight: 4 },
                    { publicKey: pk10, weight: 5 },
                    { publicKey: pk11, weight: 6 },
                ],
            }),
        ).toThrowError(new Error('Max number of signers in a multisig is 10'));
    });

    it('with max weights and max threshold values', async () => {
        expect(() =>
            MultiSigPublicKey.fromPublicKeys({
                threshold: 65535,
                publicKeys: [
                    { publicKey: pk1, weight: 1 },
                    { publicKey: pk2, weight: 256 },
                    { publicKey: pk3, weight: 3 },
                ],
            }),
        ).toThrow(new Error('Invalid u8 value: 256. Expected value in range 0-255'));

        expect(() =>
            MultiSigPublicKey.fromPublicKeys({
                threshold: 65536,
                publicKeys: [
                    { publicKey: pk1, weight: 1 },
                    { publicKey: pk2, weight: 2 },
                    { publicKey: pk3, weight: 3 },
                ],
            }),
        ).toThrow(new Error('Invalid u16 value: 65536. Expected value in range 0-65535'));
    });

    it('with zero weight value', async () => {
        expect(() =>
            MultiSigPublicKey.fromPublicKeys({
                threshold: 10,
                publicKeys: [
                    { publicKey: pk1, weight: 0 },
                    { publicKey: pk2, weight: 6 },
                    { publicKey: pk3, weight: 10 },
                ],
            }),
        ).toThrow(new Error('Invalid weight'));
    });

    it('with zero threshold value', async () => {
        expect(() =>
            MultiSigPublicKey.fromPublicKeys({
                threshold: 0,
                publicKeys: [
                    { publicKey: pk1, weight: 1 },
                    { publicKey: pk2, weight: 2 },
                    { publicKey: pk3, weight: 3 },
                ],
            }),
        ).toThrow(new Error('Invalid threshold'));
    });

    it('with empty values', async () => {
        expect(() =>
            MultiSigPublicKey.fromPublicKeys({
                threshold: 2,
                publicKeys: [],
            }),
        ).toThrow(new Error('Unreachable threshold'));
    });

    it('with duplicated publickeys', async () => {
        expect(() =>
            MultiSigPublicKey.fromPublicKeys({
                threshold: 4,
                publicKeys: [
                    { publicKey: pk1, weight: 1 },
                    { publicKey: pk1, weight: 2 },
                    { publicKey: pk3, weight: 3 },
                ],
            }),
        ).toThrow(new Error('Multisig does not support duplicate public keys'));
    });
});
