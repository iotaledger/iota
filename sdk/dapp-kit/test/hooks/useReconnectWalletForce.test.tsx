// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { renderHook, act, waitFor } from '@testing-library/react';
import { Mock, vi } from 'vitest';
import { useReconnectForceWallet, useCurrentWallet } from '../../src';
import { createWalletProviderContextWrapper, registerMockWallet } from '../test-utils';
import {
    WalletFeatureNotSupportedError,
    WalletNotConnectedError,
} from '../../src/errors/walletErrors';

vi.mock('../../src/hooks/wallet/useCurrentWallet');

describe('useReconnectForceWallet', () => {
    test('should reconnect the wallet successfully', async () => {
        const { mockWallet, unregister } = registerMockWallet({
            walletName: 'Mock Wallet 1',
            features: {
                'iota:reconnectForce': {
                    reconnect: vi.fn().mockResolvedValue({
                        accounts: [{ chains: ['iota:123'], address: 'address1' }],
                    }),
                },
            },
        });

        (useCurrentWallet as Mock).mockReturnValue({ currentWallet: mockWallet });

        const wrapper = createWalletProviderContextWrapper();
        const { result } = renderHook(
            () => ({
                currentWallet: useCurrentWallet(),
                reconnectForceWallet: useReconnectForceWallet(),
            }),
            { wrapper },
        );

        result.current.reconnectForceWallet.mutate();

        await waitFor(() => expect(result.current.reconnectForceWallet.isSuccess).toBe(true));
        expect(mockWallet.features['iota:reconnectForce']?.reconnect).toHaveBeenCalledWith({
            origin: window.location.origin,
        });

        act(() => {
            unregister();
        });
    });

    test('should throw WalletFeatureNotSupportedError if feature is not supported', async () => {
        const { mockWallet, unregister } = registerMockWallet({
            walletName: 'Mock Wallet 1',
            features: {},
        });

        (useCurrentWallet as Mock).mockReturnValue({ currentWallet: mockWallet });

        const wrapper = createWalletProviderContextWrapper();
        const { result } = renderHook(
            () => ({
                currentWallet: useCurrentWallet(),
                reconnectForceWallet: useReconnectForceWallet(),
            }),
            { wrapper },
        );

        result.current.reconnectForceWallet.mutate();

        await waitFor(() => expect(result.current.reconnectForceWallet.isError).toBe(true));

        expect(result.current.reconnectForceWallet.error).toBeInstanceOf(
            WalletFeatureNotSupportedError,
        );

        await act(async () => {
            unregister();
        });
    });

    test('should throw WalletNotConnectedError if no wallet is connected', async () => {
        (useCurrentWallet as Mock).mockReturnValue({ currentWallet: null });

        const wrapper = createWalletProviderContextWrapper();
        const { result } = renderHook(
            () => ({
                currentWallet: useCurrentWallet(),
                reconnectForceWallet: useReconnectForceWallet(),
            }),
            { wrapper },
        );

        result.current.reconnectForceWallet.mutate();

        await waitFor(() => expect(result.current.reconnectForceWallet.isError).toBe(true));
        expect(result.current.reconnectForceWallet.error).toBeInstanceOf(WalletNotConnectedError);
    });
});
