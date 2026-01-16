// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it, vi } from 'vitest';

import type { IotaObjectData } from '../../src/client/index.js';
import {
    MoveAuthenticatorBuilder,
    MoveSigner,
} from '../../src/keypairs/move-authenticator/index.js';
import { Transaction } from '../../src/transactions/index.js';
import { setup } from './utils/setup.js';
import { SIGNATURE_FLAG_TO_SCHEME } from '../../src/cryptography/signature-scheme.js';

describe('MoveAuthenticator', () => {
    it('should build, sign, and execute a transaction with MoveAuthenticator', async () => {
        const toolbox = await setup();

        // TODO: USE REAL VALUES
        // Fake account object ID (this would be a real AA account in production)
        const fakeAccountId = '0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef';

        // Fake shared object ID for testing
        const fakeSharedObjectId =
            '0xfedcba0987654321fedcba0987654321fedcba0987654321fedcba0987654321';

        // TODO: USE REAL CALL
        // Mock the client.getObject method to return fake data
        const originalGetObject = toolbox.client.getObject.bind(toolbox.client);
        vi.spyOn(toolbox.client, 'getObject').mockImplementation(async (params) => {
            if (params.id === fakeAccountId) {
                // Return a fake shared account object
                return {
                    data: {
                        objectId: fakeAccountId,
                        version: '1',
                        digest: 'FakeAccountDigest123',
                        owner: {
                            Shared: {
                                initial_shared_version: '1',
                            },
                        },
                    } as IotaObjectData,
                } as any;
            } else if (params.id === fakeSharedObjectId) {
                // Return a fake shared object
                return {
                    data: {
                        objectId: fakeSharedObjectId,
                        version: '5',
                        digest: 'FakeSharedObjectDigest456',
                        owner: {
                            Shared: {
                                initial_shared_version: '1',
                            },
                        },
                    } as IotaObjectData,
                } as any;
            }
            // Fall back to original implementation for other objects
            return originalGetObject(params);
        });

        // TODO: USE REAL VALUES
        // 1. Build the MoveAuthenticator
        const builder = new MoveAuthenticatorBuilder(fakeAccountId)
            .addSharedObject(fakeSharedObjectId, true)
            .addPure(new Uint8Array([1, 2, 3, 4]))
            .addTypeArg('0x2::iota::IOTA');

        // 2. Resolve the authenticator data
        const authenticatorData = await builder.finish(toolbox.client);
        expect(authenticatorData).toBeDefined();

        // 3. Create a MoveSigner
        const signer = new MoveSigner(authenticatorData);

        // Verify signer properties
        expect(signer.getKeyScheme()).toBe('MoveAuthenticator');
        const publicKey = signer.getPublicKey();
        expect(publicKey).toBeDefined();

        // 4. Create and sign a transaction
        const tx = new Transaction();
        tx.setSender(fakeAccountId);

        // Add a simple transfer to the transaction
        const coin = tx.splitCoins(tx.gas, [1000]);
        tx.transferObjects([coin], toolbox.address());

        // Build the transaction bytes
        await tx.build({ client: toolbox.client });
        const txBytes = await tx.toJSON();

        // Sign the transaction with MoveSigner
        const { signature, bytes } = await signer.signTransaction(
            new Uint8Array(Buffer.from(txBytes)),
        );
        expect(signature).toBeDefined();
        expect(bytes).toBeDefined();

        // Verify the signature starts with the MoveAuthenticator flag (0x07)
        const signatureBytes = Buffer.from(signature, 'base64');
        expect(signatureBytes[0]).toBe(SIGNATURE_FLAG_TO_SCHEME[7]);

        console.log('MoveAuthenticator signature:', signature);
        console.log('Transaction bytes:', bytes);

        // TODO: SETUP A REAL AA
        const result = await toolbox.client.executeTransactionBlock({
            transactionBlock: bytes,
            signature,
        });

        expect(result.effects?.status.status).toEqual('success');

        // Restore the original getObject method
        vi.restoreAllMocks();
    });

    it('should throw error for invalid account (owned)', async () => {
        const toolbox = await setup();

        const fakeOwnedAccountId =
            '0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';

        // Mock getObject to return an owned account
        vi.spyOn(toolbox.client, 'getObject').mockResolvedValue({
            data: {
                objectId: fakeOwnedAccountId,
                version: '1',
                digest: 'FakeDigest',
                owner: {
                    AddressOwner: toolbox.address(),
                },
            } as IotaObjectData,
        } as any);

        const builder = new MoveAuthenticatorBuilder(fakeOwnedAccountId);

        // Should throw because account is owned, not shared or immutable
        await expect(builder.finish(toolbox.client)).rejects.toThrow(
            'account must be immutable or shared',
        );

        vi.restoreAllMocks();
    });

    it('should handle immutable account', async () => {
        const toolbox = await setup();

        const fakeImmutableAccountId =
            '0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';

        // Mock getObject to return an immutable account
        vi.spyOn(toolbox.client, 'getObject').mockResolvedValue({
            data: {
                objectId: fakeImmutableAccountId,
                version: '1',
                digest: 'ImmutableDigest',
                owner: 'Immutable',
            } as IotaObjectData,
        } as any);

        const builder = new MoveAuthenticatorBuilder(fakeImmutableAccountId);
        const data = await builder.finish(toolbox.client);

        expect(data.objectToAuthenticate.$kind).toBe('Immutable');
        if (data.objectToAuthenticate.$kind === 'Immutable') {
            expect(data.objectToAuthenticate.Immutable.objectId).toBe(fakeImmutableAccountId);
            expect(data.objectToAuthenticate.Immutable.version).toBe('1');
            expect(data.objectToAuthenticate.Immutable.digest).toBe('ImmutableDigest');
        }

        vi.restoreAllMocks();
    });

    it('should throw error when adding owned object as call arg', async () => {
        const toolbox = await setup();

        const fakeAccountId = '0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef';
        const fakeOwnedObjectId =
            '0xcccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc';

        vi.spyOn(toolbox.client, 'getObject').mockImplementation(async (params) => {
            if (params.id === fakeAccountId) {
                return {
                    data: {
                        objectId: fakeAccountId,
                        version: '1',
                        digest: 'AccountDigest',
                        owner: 'Immutable',
                    } as IotaObjectData,
                } as any;
            } else if (params.id === fakeOwnedObjectId) {
                return {
                    data: {
                        objectId: fakeOwnedObjectId,
                        version: '1',
                        digest: 'OwnedDigest',
                        owner: {
                            AddressOwner: toolbox.address(),
                        },
                    } as IotaObjectData,
                } as any;
            }
            throw new Error('Object not found');
        });

        const builder = new MoveAuthenticatorBuilder(fakeAccountId).addImmutableObject(
            fakeOwnedObjectId,
        );

        // Should throw because the object is owned, not immutable
        await expect(builder.finish(toolbox.client)).rejects.toThrow(
            'call arguments must not be owned',
        );

        vi.restoreAllMocks();
    });
});
