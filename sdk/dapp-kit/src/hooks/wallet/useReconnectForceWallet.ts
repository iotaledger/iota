// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-nocheck
import type { UseMutationOptions, UseMutationResult } from '@tanstack/react-query';
import { useMutation } from '@tanstack/react-query';

import { walletMutationKeys } from '../../constants/walletMutationKeys.js';
import { WalletNotConnectedError } from '../../errors/walletErrors.js';
import { useCurrentWallet } from './useCurrentWallet.js';
import { useWalletStore } from './useWalletStore.js';
import { isSupportedChain } from '@mysten/wallet-standard';
import type { WalletAccount } from '@mysten/wallet-standard';

type UseDisconnectWalletError = WalletNotConnectedError | Error;

type UseDisconnectWalletMutationOptions = Omit<
    UseMutationOptions<void, UseDisconnectWalletError, void, unknown>,
    'mutationFn'
>;

/**
 * Mutation hook for disconnecting from an active wallet connection, if currently connected.
 */
export function useReconnectForceWallet({
    mutationKey,
    ...mutationOptions
}: UseDisconnectWalletMutationOptions = {}): UseMutationResult<
    void,
    UseDisconnectWalletError,
    void
> {
    const { currentWallet } = useCurrentWallet();
    const setWalletConnected = useWalletStore((state) => state.setWalletConnected);

    return useMutation({
        mutationKey: walletMutationKeys.disconnectAllWallet(mutationKey),
        mutationFn: async () => {
            if (!currentWallet) {
                throw new WalletNotConnectedError('No wallet is connected.');
            }

            try {
                // Wallets aren't required to implement the disconnect feature, so we'll
                // optionally call the disconnect feature if it exists and reset the UI
                // state on the frontend at a minimum.
                const connectResult = await currentWallet.features[
                    'standard:reconnectForce'
                ]?.reconnect({
                    origin: window.location.origin,
                });
                const connectedSuiAccounts = connectResult.accounts.filter((account) =>
                    account.chains.some(isSupportedChain),
                );
                const selectedAccount = getSelectedAccount(connectedSuiAccounts);

                setWalletConnected(wallet, connectedSuiAccounts, selectedAccount);

                return { accounts: connectedSuiAccounts };
            } catch (error) {
                console.error(
                    'Failed to disconnect the application from the current wallet.',
                    error,
                );
            }
        },
        ...mutationOptions,
    });
}

function getSelectedAccount(connectedAccounts: readonly WalletAccount[], accountAddress?: string) {
    if (connectedAccounts.length === 0) {
        return null;
    }

    if (accountAddress) {
        const selectedAccount = connectedAccounts.find(
            (account) => account.address === accountAddress,
        );
        return selectedAccount ?? connectedAccounts[0];
    }

    return connectedAccounts[0];
}
