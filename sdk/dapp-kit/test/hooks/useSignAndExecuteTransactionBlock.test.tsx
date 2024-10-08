// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';
import { TransactionBlock } from '@iota/iota-sdk/transactions';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { Mock } from 'vitest';

import {
    WalletFeatureNotSupportedError,
    WalletNotConnectedError,
} from '../../src/errors/walletErrors.js';
import { useConnectWallet, useSignAndExecuteTransactionBlock } from '../../src/index.js';
import { iotaFeatures } from '../mocks/mockFeatures.js';
import { createWalletProviderContextWrapper, registerMockWallet } from '../test-utils.js';

describe('useSignAndExecuteTransactionBlock', () => {
    test('throws an error when trying to sign and execute a transaction block without a wallet connection', async () => {
        const wrapper = createWalletProviderContextWrapper();
        const { result } = renderHook(() => useSignAndExecuteTransactionBlock(), { wrapper });

        result.current.mutate({ transactionBlock: new TransactionBlock(), chain: 'iota:testnet' });

        await waitFor(() => expect(result.current.error).toBeInstanceOf(WalletNotConnectedError));
    });

    test('throws an error when trying to sign and execute a transaction block with a wallet that lacks feature support', async () => {
        const { unregister, mockWallet } = registerMockWallet({
            walletName: 'Mock Wallet 1',
        });

        const wrapper = createWalletProviderContextWrapper();
        const { result } = renderHook(
            () => ({
                connectWallet: useConnectWallet(),
                useSignAndExecuteTransactionBlock: useSignAndExecuteTransactionBlock(),
            }),
            { wrapper },
        );

        result.current.connectWallet.mutate({ wallet: mockWallet });
        await waitFor(() => expect(result.current.connectWallet.isSuccess).toBe(true));

        result.current.useSignAndExecuteTransactionBlock.mutate({
            transactionBlock: new TransactionBlock(),
            chain: 'iota:testnet',
        });
        await waitFor(() =>
            expect(result.current.useSignAndExecuteTransactionBlock.error).toBeInstanceOf(
                WalletFeatureNotSupportedError,
            ),
        );

        act(() => unregister());
    });

    test('signing and executing a transaction block from the currently connected account works successfully', async () => {
        const { unregister, mockWallet } = registerMockWallet({
            walletName: 'Mock Wallet 1',
            features: iotaFeatures,
        });

        const iotaClient = new IotaClient({ url: getFullnodeUrl('localnet') });
        const executeTransactionBlock = vi.spyOn(iotaClient, 'executeTransactionBlock');

        executeTransactionBlock.mockReturnValueOnce(Promise.resolve({ digest: '123' }));

        const wrapper = createWalletProviderContextWrapper({}, iotaClient);
        const { result } = renderHook(
            () => ({
                connectWallet: useConnectWallet(),
                useSignAndExecuteTransactionBlock: useSignAndExecuteTransactionBlock(),
            }),
            { wrapper },
        );

        result.current.connectWallet.mutate({ wallet: mockWallet });

        await waitFor(() => expect(result.current.connectWallet.isSuccess).toBe(true));

        const signTransactionBlockFeature = mockWallet.features['iota:signTransactionBlock'];
        const signTransactionBlockMock = signTransactionBlockFeature!.signTransactionBlock as Mock;

        signTransactionBlockMock.mockReturnValueOnce({
            transactionBlockBytes: 'abc',
            signature: '123',
        });

        result.current.useSignAndExecuteTransactionBlock.mutate({
            transactionBlock: new TransactionBlock(),
            chain: 'iota:testnet',
        });

        await waitFor(() =>
            expect(result.current.useSignAndExecuteTransactionBlock.isSuccess).toBe(true),
        );
        expect(result.current.useSignAndExecuteTransactionBlock.data).toStrictEqual({
            digest: '123',
        });
        expect(iotaClient.executeTransactionBlock).toHaveBeenCalledWith({
            transactionBlock: 'abc',
            signature: '123',
        });

        act(() => unregister());
    });
});
