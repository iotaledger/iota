// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from 'vitest';

import { Ed25519Keypair } from '../../src/keypairs/ed25519';
import { Secp256k1Keypair } from '../../src/keypairs/secp256k1';
import { Secp256r1Keypair } from '../../src/keypairs/secp256r1';
import { MultiSigPublicKey } from '../../src/multisig/publickey';
import { verifyPersonalMessageSignature } from '../../src/verify';

describe('Verify Signatures', () => {
    // describe('transaction signatures', () => {});
    describe('personal message signatures', () => {
        describe('single signatures', () => {
            describe('Ed25519', () => {
                const keypair = new Ed25519Keypair();
                const address = keypair.getPublicKey().toIotaAddress();
                const message = new TextEncoder().encode('hello world');

                it('verifies valid signatures', async () => {
                    const { signature } = await keypair.signPersonalMessage(message);
                    const publicKey = await verifyPersonalMessageSignature(message, signature);
                    expect(publicKey.toIotaAddress()).toBe(address);
                });

                it('verifies signatures against provided address', async () => {
                    const { signature } = await keypair.signPersonalMessage(message);
                    await expect(
                        verifyPersonalMessageSignature(message, signature, { address }),
                    ).resolves.toBeDefined();
                });

                it('fails for invalid signatures', async () => {
                    const { signature } = await keypair.signPersonalMessage(message);
                    const invalidMessage = new TextEncoder().encode('wrong message');
                    await expect(
                        verifyPersonalMessageSignature(invalidMessage, signature),
                    ).rejects.toThrow();
                });

                it('fails for wrong address', async () => {
                    const { signature } = await keypair.signPersonalMessage(message);
                    const wrongAddress = new Ed25519Keypair().getPublicKey().toIotaAddress();
                    await expect(
                        verifyPersonalMessageSignature(message, signature, {
                            address: wrongAddress,
                        }),
                    ).rejects.toThrow();
                });
            });

            describe('Secp256k1', () => {
                const keypair = new Secp256k1Keypair();
                const address = keypair.getPublicKey().toIotaAddress();
                const message = new TextEncoder().encode('hello world');

                it('verifies valid signatures', async () => {
                    const { signature } = await keypair.signPersonalMessage(message);
                    const publicKey = await verifyPersonalMessageSignature(message, signature);
                    expect(publicKey.toIotaAddress()).toBe(address);
                });

                it('verifies signatures against provided address', async () => {
                    const { signature } = await keypair.signPersonalMessage(message);
                    await expect(
                        verifyPersonalMessageSignature(message, signature, { address }),
                    ).resolves.toBeDefined();
                });

                it('fails for invalid signatures', async () => {
                    const { signature } = await keypair.signPersonalMessage(message);
                    const invalidMessage = new TextEncoder().encode('wrong message');
                    await expect(
                        verifyPersonalMessageSignature(invalidMessage, signature),
                    ).rejects.toThrow();
                });

                it('fails for wrong address', async () => {
                    const { signature } = await keypair.signPersonalMessage(message);
                    const wrongAddress = new Secp256k1Keypair().getPublicKey().toIotaAddress();
                    await expect(
                        verifyPersonalMessageSignature(message, signature, {
                            address: wrongAddress,
                        }),
                    ).rejects.toThrow();
                });
            });

            describe('Secp256r1', () => {
                const keypair = new Secp256r1Keypair();
                const address = keypair.getPublicKey().toIotaAddress();
                const message = new TextEncoder().encode('hello world');

                it('verifies valid signatures', async () => {
                    const { signature } = await keypair.signPersonalMessage(message);
                    const publicKey = await verifyPersonalMessageSignature(message, signature);
                    expect(publicKey.toIotaAddress()).toBe(address);
                });

                it('verifies signatures against provided address', async () => {
                    const { signature } = await keypair.signPersonalMessage(message);
                    await expect(
                        verifyPersonalMessageSignature(message, signature, { address }),
                    ).resolves.toBeDefined();
                });

                it('fails for invalid signatures', async () => {
                    const { signature } = await keypair.signPersonalMessage(message);
                    const invalidMessage = new TextEncoder().encode('wrong message');
                    await expect(
                        verifyPersonalMessageSignature(invalidMessage, signature),
                    ).rejects.toThrow();
                });

                it('fails for wrong address', async () => {
                    const { signature } = await keypair.signPersonalMessage(message);
                    const wrongAddress = new Secp256r1Keypair().getPublicKey().toIotaAddress();
                    await expect(
                        verifyPersonalMessageSignature(message, signature, {
                            address: wrongAddress,
                        }),
                    ).rejects.toThrow();
                });
            });
        });

        describe('multisig signatures', () => {
            const k1 = new Ed25519Keypair();
            const k2 = new Secp256k1Keypair();
            const k3 = new Secp256r1Keypair();
            const pk1 = k1.getPublicKey();
            const pk2 = k2.getPublicKey();
            const pk3 = k3.getPublicKey();

            it('verifies valid multisig signatures', async () => {
                const multiSigPublicKey = MultiSigPublicKey.fromPublicKeys({
                    threshold: 3,
                    publicKeys: [
                        { publicKey: pk1, weight: 1 },
                        { publicKey: pk2, weight: 2 },
                        { publicKey: pk3, weight: 3 },
                    ],
                });

                const message = new TextEncoder().encode('hello world');
                const sig1 = await k1.signPersonalMessage(message);
                const sig2 = await k2.signPersonalMessage(message);

                const multisig = multiSigPublicKey.combinePartialSignatures([
                    sig1.signature,
                    sig2.signature,
                ]);

                const publicKey = await verifyPersonalMessageSignature(message, multisig);
                expect(publicKey.toIotaAddress()).toBe(multiSigPublicKey.toIotaAddress());
            });

            it('fails for invalid multisig signatures', async () => {
                const multiSigPublicKey = MultiSigPublicKey.fromPublicKeys({
                    threshold: 3,
                    publicKeys: [
                        { publicKey: pk1, weight: 1 },
                        { publicKey: pk2, weight: 2 },
                        { publicKey: pk3, weight: 3 },
                    ],
                });

                const message = new TextEncoder().encode('hello world');
                const wrongMessage = new TextEncoder().encode('wrong message');
                const sig1 = await k1.signPersonalMessage(message);
                const sig2 = await k2.signPersonalMessage(message);

                const multisig = multiSigPublicKey.combinePartialSignatures([
                    sig1.signature,
                    sig2.signature,
                ]);

                await expect(
                    verifyPersonalMessageSignature(wrongMessage, multisig),
                ).rejects.toThrow();
            });
        });
    });
});
