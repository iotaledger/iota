// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { UseMutationOptions, UseMutationResult } from '@tanstack/react-query';
import { useMutation } from '@tanstack/react-query';

import { walletMutationKeys } from '../../constants/walletMutationKeys.js';
import {
    WalletFeatureNotSupportedError,
    WalletNotConnectedError,
} from '../../errors/walletErrors.js';
import { useCurrentWallet } from './useCurrentWallet.js';
import { useWalletStore } from './useWalletStore.js';
import type { IotaReconnectForceOutput } from '@iota/wallet-standard';
import { isSupportedChain } from '@iota/wallet-standard';
import { getSelectedAccount } from '../../utils/getSelectedAccount.js';

type UseReconnectForceError = WalletNotConnectedError | Error;

type ReconnectForceWalletArgs = void;

type ReconnectForceWalletResult = IotaReconnectForceOutput;

type UseReconnectForceWalletMutationOptions = Omit<
    UseMutationOptions<
        ReconnectForceWalletResult,
        UseReconnectForceError,
        ReconnectForceWalletArgs,
        undefined
    >,
    'mutationFn'
>;

/**
 * Mutation hook for reconnecting again so the user can choose what accounts to use.
 */
export function useReconnectForceWallet({
    mutationKey,
    ...mutationOptions
}: UseReconnectForceWalletMutationOptions = {}): UseMutationResult<
    ReconnectForceWalletResult,
    UseReconnectForceError,
    ReconnectForceWalletArgs,
    undefined
> {
    const { currentWallet } = useCurrentWallet();
    const setWalletConnected = useWalletStore((state) => state.setWalletConnected);
    const setConnectionStatus = useWalletStore((state) => state.setConnectionStatus);

    return useMutation({
        mutationKey: walletMutationKeys.reconnectForceWallet(mutationKey),
        mutationFn: async () => {
            if (!currentWallet) {
                throw new WalletNotConnectedError('No wallet is connected.');
            }

            try {
                setConnectionStatus('connecting');

                const walletFeature = currentWallet.features['iota:reconnectForce'];

                if (!walletFeature) {
                    throw new WalletFeatureNotSupportedError(
                        "This wallet doesn't support the `reconnectForce` feature.",
                    );
                }

                const connectResult = await walletFeature.reconnect({
                    origin: window.location.origin,
                });

                if (!connectResult) {
                    throw new Error('Connect result is undefined');
                }

                const connectedSuiAccounts = connectResult.accounts.filter((account) =>
                    account.chains.some(isSupportedChain),
                );
                const selectedAccount = getSelectedAccount(connectedSuiAccounts);

                setWalletConnected(currentWallet, connectedSuiAccounts, selectedAccount);

                return { accounts: connectedSuiAccounts };
            } catch (error) {
                setConnectionStatus('disconnected');
                throw error;
            }
        },
        ...mutationOptions,
    });
}
