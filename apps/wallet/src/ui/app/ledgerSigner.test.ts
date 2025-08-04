// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, it, expect, vi, beforeEach, type MockedFunction } from 'vitest';
import type IotaLedgerClient from '@iota/ledgerjs-hw-app-iota';
import { IotaClient } from '@iota/iota-sdk/client';
import { Ed25519PublicKey } from '@iota/iota-sdk/keypairs/ed25519';
import { Transaction } from '@iota/iota-sdk/transactions';

import { LedgerSigner } from './ledgerSigner';

// Mock dependencies
vi.mock('@iota/signers/ledger', () => ({
    LedgerSigner: {
        fromDerivationPath: vi.fn(),
    },
}));

vi.mock('@iota/iota-sdk/client');
vi.mock('@iota/iota-sdk/keypairs/ed25519');
vi.mock('@iota/iota-sdk/transactions', () => ({
    Transaction: vi.fn(),
    isTransaction: vi.fn(),
}));

describe('LedgerSigner', () => {
    let mockIotaLedgerClient: IotaLedgerClient;
    let mockConnectToLedger: MockedFunction<() => Promise<IotaLedgerClient>>;
    let mockIotaClient: IotaClient;
    let mockSignersLedgerSigner: any;
    let ledgerSigner: LedgerSigner;
    let mockPublicKey: Ed25519PublicKey;

    const derivationPath = "m/44'/4218'/0'/0'/0'";
    const testAddress = 'iota1qpszqzadsym6wpppd6z037dvlejmnerbc9f4tu3nqfxvgtvk4smna9w5jmj';
    const testSignature = 'test-signature';
    const testBytes = 'test-bytes';

    beforeEach(async () => {
        vi.clearAllMocks();

        // Mock IotaLedgerClient
        mockIotaLedgerClient = {} as IotaLedgerClient;

        // Mock connect function
        mockConnectToLedger = vi.fn().mockResolvedValue(mockIotaLedgerClient);

        // Mock IotaClient
        mockIotaClient = {} as IotaClient;

        // Mock Ed25519PublicKey
        mockPublicKey = {} as Ed25519PublicKey;

        // Mock SignersLedgerSigner instance
        mockSignersLedgerSigner = {
            toIotaAddress: vi.fn().mockResolvedValue(testAddress),
            getPublicKey: vi.fn().mockResolvedValue(mockPublicKey),
            signPersonalMessage: vi
                .fn()
                .mockResolvedValue({ bytes: testBytes, signature: testSignature }),
            signTransaction: vi
                .fn()
                .mockResolvedValue({ bytes: testBytes, signature: testSignature }),
        };

        // Import and mock the static fromDerivationPath method
        const { LedgerSigner: MockedLedgerSigner } = await import('@iota/signers/ledger');
        vi.mocked(MockedLedgerSigner.fromDerivationPath).mockResolvedValue(mockSignersLedgerSigner);

        // Create LedgerSigner instance
        ledgerSigner = new LedgerSigner(
            mockConnectToLedger,
            derivationPath,
            testAddress,
            mockIotaClient,
        );
    });

    describe('constructor', () => {
        it('should initialize with correct parameters', () => {
            expect(ledgerSigner).toBeInstanceOf(LedgerSigner);
            expect(ledgerSigner.client).toBe(mockIotaClient);
        });

        it('should accept derivation path parameter', () => {
            const customPath = "m/44'/4218'/1'/0'/0'";
            const customSigner = new LedgerSigner(
                mockConnectToLedger,
                customPath,
                testAddress,
                mockIotaClient,
            );
            expect(customSigner).toBeInstanceOf(LedgerSigner);
        });
    });

    describe('ledger client initialization', () => {
        it('should initialize ledger client on first call', async () => {
            await ledgerSigner.getAddress();

            expect(mockConnectToLedger).toHaveBeenCalledTimes(1);
        });

        it('should cache ledger client after first initialization', async () => {
            await ledgerSigner.getAddress();
            await ledgerSigner.getAddress();

            expect(mockConnectToLedger).toHaveBeenCalledTimes(1);
        });

        it('should handle ledger client initialization error', async () => {
            const error = new Error('Failed to connect to ledger');
            mockConnectToLedger.mockRejectedValue(error);

            await expect(ledgerSigner.getAddress()).rejects.toThrow('Failed to connect to ledger');
        });
    });

    describe('signer initialization', () => {
        it('should initialize signer with correct parameters', async () => {
            await ledgerSigner.getAddress();

            const { LedgerSigner: MockedLedgerSigner } = await import('@iota/signers/ledger');
            expect(MockedLedgerSigner.fromDerivationPath).toHaveBeenCalledWith(
                derivationPath,
                mockIotaLedgerClient,
                mockIotaClient,
            );
        });

        it('should cache signer after first initialization', async () => {
            await ledgerSigner.getAddress();
            await ledgerSigner.getPublicKey();

            const { LedgerSigner: MockedLedgerSigner } = await import('@iota/signers/ledger');
            expect(MockedLedgerSigner.fromDerivationPath).toHaveBeenCalledTimes(1);
        });

        it('should handle signer initialization error', async () => {
            const error = new Error('Failed to create signer');
            const { LedgerSigner: MockedLedgerSigner } = await import('@iota/signers/ledger');
            vi.mocked(MockedLedgerSigner.fromDerivationPath).mockRejectedValue(error);

            await expect(ledgerSigner.getAddress()).rejects.toThrow('Failed to create signer');
        });
    });

    describe('getAddress', () => {
        it('should return address from signer', async () => {
            const address = await ledgerSigner.getAddress();

            expect(address).toBe(testAddress);
            expect(mockSignersLedgerSigner.toIotaAddress).toHaveBeenCalledTimes(1);
        });

        it('should handle address retrieval error', async () => {
            const error = new Error('Failed to get address');
            mockSignersLedgerSigner.toIotaAddress.mockRejectedValue(error);

            await expect(ledgerSigner.getAddress()).rejects.toThrow('Failed to get address');
        });
    });

    describe('getPublicKey', () => {
        it('should return public key from signer', async () => {
            const publicKey = await ledgerSigner.getPublicKey();

            expect(publicKey).toBe(mockPublicKey);
            expect(mockSignersLedgerSigner.getPublicKey).toHaveBeenCalledTimes(1);
        });

        it('should handle public key retrieval error', async () => {
            const error = new Error('Failed to get public key');
            mockSignersLedgerSigner.getPublicKey.mockRejectedValue(error);

            await expect(ledgerSigner.getPublicKey()).rejects.toThrow('Failed to get public key');
        });
    });

    describe('signMessage', () => {
        it('should sign message using signer', async () => {
            const message = new Uint8Array([1, 2, 3, 4, 5]);
            const result = await ledgerSigner.signMessage({ message });

            expect(result).toEqual({ bytes: testBytes, signature: testSignature });
            expect(mockSignersLedgerSigner.signPersonalMessage).toHaveBeenCalledWith(message);
        });

        it('should handle message signing error', async () => {
            const error = new Error('Failed to sign message');
            mockSignersLedgerSigner.signPersonalMessage.mockRejectedValue(error);

            const message = new Uint8Array([1, 2, 3, 4, 5]);
            await expect(ledgerSigner.signMessage({ message })).rejects.toThrow(
                'Failed to sign message',
            );
        });
    });

    describe('signTransaction', () => {
        it('should sign transaction with Uint8Array', async () => {
            const transaction = new Uint8Array([1, 2, 3, 4, 5]);

            const result = await ledgerSigner.signTransaction({ transaction });

            expect(result).toEqual({ bytes: testBytes, signature: testSignature });
            expect(mockSignersLedgerSigner.signTransaction).toHaveBeenCalledWith(transaction);
        });

        it('should sign transaction with Transaction object', async () => {
            const { isTransaction } = await import('@iota/iota-sdk/transactions');
            vi.mocked(isTransaction).mockReturnValue(true);

            const mockTransaction = {
                getData: vi.fn().mockReturnValue({ sender: 'test-sender' }),
                build: vi.fn().mockResolvedValue(new Uint8Array([1, 2, 3, 4, 5])),
            } as unknown as Transaction;

            const result = await ledgerSigner.signTransaction({ transaction: mockTransaction });

            expect(result).toEqual({ bytes: testBytes, signature: testSignature });
            expect(mockSignersLedgerSigner.signTransaction).toHaveBeenCalled();
        });

        it('should handle transaction signing error', async () => {
            const error = new Error('Failed to sign transaction');
            mockSignersLedgerSigner.signTransaction.mockRejectedValue(error);

            const transaction = new Uint8Array([1, 2, 3, 4, 5]);

            await expect(ledgerSigner.signTransaction({ transaction })).rejects.toThrow(
                'Failed to sign transaction',
            );
        });
    });

    describe('address verification', () => {
        it('should verify correct address before signing transaction', async () => {
            const transaction = new Uint8Array([1, 2, 3, 4, 5]);

            const result = await ledgerSigner.signTransaction({ transaction });

            expect(result).toEqual({ bytes: testBytes, signature: testSignature });
            // Verify that getAddress was called for verification
            expect(mockSignersLedgerSigner.toIotaAddress).toHaveBeenCalled();
        });

        it('should verify correct address before signing message', async () => {
            const message = new Uint8Array([1, 2, 3, 4, 5]);

            const result = await ledgerSigner.signMessage({ message });

            expect(result).toEqual({ bytes: testBytes, signature: testSignature });
            // Verify that getAddress was called for verification
            expect(mockSignersLedgerSigner.toIotaAddress).toHaveBeenCalled();
        });

        it('should throw error when ledger address does not match expected address', async () => {
            const wrongAddress =
                'iota1qppppppppppppppppppppppppppppppppppppppppppppppppppppppppu7xss';
            mockSignersLedgerSigner.toIotaAddress.mockResolvedValue(wrongAddress);

            const transaction = new Uint8Array([1, 2, 3, 4, 5]);

            await expect(ledgerSigner.signTransaction({ transaction })).rejects.toThrow(
                `Ledger address mismatch. Expected: ${testAddress}, Got: ${wrongAddress}`,
            );

            // Should not proceed to actual signing
            expect(mockSignersLedgerSigner.signTransaction).not.toHaveBeenCalled();
        });

        it('should throw error when ledger address verification fails during message signing', async () => {
            const wrongAddress =
                'iota1qppppppppppppppppppppppppppppppppppppppppppppppppppppppppu7xss';
            mockSignersLedgerSigner.toIotaAddress.mockResolvedValue(wrongAddress);

            const message = new Uint8Array([1, 2, 3, 4, 5]);

            await expect(ledgerSigner.signMessage({ message })).rejects.toThrow(
                `Ledger address mismatch. Expected: ${testAddress}, Got: ${wrongAddress}`,
            );

            // Should not proceed to actual signing
            expect(mockSignersLedgerSigner.signPersonalMessage).not.toHaveBeenCalled();
        });
    });

    describe('connect', () => {
        it('should return new LedgerSigner instance with new client', () => {
            const newClient = {} as IotaClient;
            const newSigner = ledgerSigner.connect(newClient);

            expect(newSigner).toBeInstanceOf(LedgerSigner);
            expect(newSigner).not.toBe(ledgerSigner);
            expect(newSigner.client).toBe(newClient);
        });

        it('should preserve derivation path and connect function', () => {
            const newClient = {} as IotaClient;
            const newSigner = ledgerSigner.connect(newClient);

            // The new signer should be able to use the same connection and derivation path
            expect(newSigner).toBeInstanceOf(LedgerSigner);
        });
    });

    describe('error handling', () => {
        it('should handle multiple concurrent operations', async () => {
            // Start multiple operations that require initialization
            const promises = [
                ledgerSigner.getAddress(),
                ledgerSigner.getPublicKey(),
                ledgerSigner.signMessage({ message: new Uint8Array([1, 2, 3]) }),
            ];

            const results = await Promise.all(promises);

            // All operations should complete successfully
            expect(results[0]).toBe(testAddress);
            expect(results[1]).toBe(mockPublicKey);
            expect(results[2]).toEqual({ bytes: testBytes, signature: testSignature });
        });

        it('should handle ledger disconnection gracefully', async () => {
            // First successful call
            await ledgerSigner.getAddress();

            // Simulate disconnection by making connect fail
            const error = new Error('Ledger disconnected');
            mockConnectToLedger.mockRejectedValue(error);

            // Create new signer instance to test reconnection
            const newSigner = new LedgerSigner(
                mockConnectToLedger,
                derivationPath,
                testAddress,
                mockIotaClient,
            );

            await expect(newSigner.getAddress()).rejects.toThrow('Ledger disconnected');
        });

        it('should handle user rejection during signing', async () => {
            const userRejectionError = new Error('User rejected the request');
            mockSignersLedgerSigner.signTransaction.mockRejectedValue(userRejectionError);

            const transaction = new Uint8Array([1, 2, 3, 4, 5]);

            await expect(ledgerSigner.signTransaction({ transaction })).rejects.toThrow(
                'User rejected the request',
            );
        });

        it('should handle device locked error', async () => {
            const deviceLockedError = new Error('Device is locked');
            mockConnectToLedger.mockRejectedValue(deviceLockedError);

            await expect(ledgerSigner.getAddress()).rejects.toThrow('Device is locked');
        });
    });

    describe('transaction preparation', () => {
        it('should handle string transaction input', async () => {
            const { isTransaction } = await import('@iota/iota-sdk/transactions');
            vi.mocked(isTransaction).mockReturnValue(false);

            const stringTransaction = 'base64encodedtransaction';
            const mockFromB64 = vi.fn().mockReturnValue(new Uint8Array([1, 2, 3, 4, 5]));

            // Mock fromB64 from utils
            vi.doMock('@iota/iota-sdk/utils', () => ({
                fromB64: mockFromB64,
            }));

            const result = await ledgerSigner.signTransaction({
                transaction: stringTransaction as any,
            });

            expect(result).toEqual({ bytes: testBytes, signature: testSignature });
        });

        it('should handle transaction building failure', async () => {
            const { isTransaction } = await import('@iota/iota-sdk/transactions');
            vi.mocked(isTransaction).mockReturnValue(true);

            const mockTransaction = {
                getData: vi.fn().mockReturnValue({ sender: null }),
                setSender: vi.fn(),
                build: vi.fn().mockRejectedValue(new Error('Build failed')),
            } as unknown as Transaction;

            await expect(
                ledgerSigner.signTransaction({ transaction: mockTransaction }),
            ).rejects.toThrow('Build failed');
        });

        it('should set sender on transaction when not already set', async () => {
            const { isTransaction } = await import('@iota/iota-sdk/transactions');
            vi.mocked(isTransaction).mockReturnValue(true);

            const mockTransaction = {
                getData: vi.fn().mockReturnValue({ sender: null }),
                setSender: vi.fn(),
                build: vi.fn().mockResolvedValue(new Uint8Array([1, 2, 3, 4, 5])),
            } as unknown as Transaction;

            await ledgerSigner.signTransaction({ transaction: mockTransaction });

            expect(mockTransaction.setSender).toHaveBeenCalledWith(testAddress);
            expect(mockTransaction.build).toHaveBeenCalledWith({ client: mockIotaClient });
        });

        it('should not overwrite existing sender on transaction', async () => {
            const { isTransaction } = await import('@iota/iota-sdk/transactions');
            vi.mocked(isTransaction).mockReturnValue(true);

            const existingSender = 'existing-sender-address';
            const mockTransaction = {
                getData: vi.fn().mockReturnValue({ sender: existingSender }),
                setSender: vi.fn(),
                build: vi.fn().mockResolvedValue(new Uint8Array([1, 2, 3, 4, 5])),
            } as unknown as Transaction;

            await ledgerSigner.signTransaction({ transaction: mockTransaction });

            expect(mockTransaction.setSender).not.toHaveBeenCalled();
            expect(mockTransaction.build).toHaveBeenCalledWith({ client: mockIotaClient });
        });
    });

    describe('derivation path validation', () => {
        it('should accept valid BIP44 derivation paths', () => {
            const validPaths = [
                "m/44'/4218'/0'/0'/0'",
                "m/44'/4218'/1'/0'/0'",
                "m/44'/4218'/0'/0'/1'",
            ];

            validPaths.forEach((path) => {
                expect(
                    () => new LedgerSigner(mockConnectToLedger, path, testAddress, mockIotaClient),
                ).not.toThrow();
            });
        });

        it('should pass correct derivation path to SignersLedgerSigner', async () => {
            const customPath = "m/44'/4218'/1'/0'/0'";
            const customSigner = new LedgerSigner(
                mockConnectToLedger,
                customPath,
                testAddress,
                mockIotaClient,
            );

            await customSigner.getAddress();

            const { LedgerSigner: MockedLedgerSigner } = await import('@iota/signers/ledger');
            expect(MockedLedgerSigner.fromDerivationPath).toHaveBeenCalledWith(
                customPath,
                mockIotaLedgerClient,
                mockIotaClient,
            );
        });
    });

    describe('initialization order', () => {
        it('should initialize ledger client before signer', async () => {
            const initOrder: string[] = [];

            mockConnectToLedger.mockImplementation(async () => {
                initOrder.push('ledger-client');
                return mockIotaLedgerClient;
            });

            const { LedgerSigner: MockedLedgerSigner } = await import('@iota/signers/ledger');
            vi.mocked(MockedLedgerSigner.fromDerivationPath).mockImplementation(async () => {
                initOrder.push('signer');
                return mockSignersLedgerSigner;
            });

            await ledgerSigner.getAddress();

            expect(initOrder).toEqual(['ledger-client', 'signer']);
        });
    });
});
