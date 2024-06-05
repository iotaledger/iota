// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { UseMutationOptions, UseMutationResult } from '@tanstack/react-query';
import { useMutation } from '@tanstack/react-query';

import { walletMutationKeys } from '../../constants/walletMutationKeys.js';
import { WalletNotConnectedError } from '../../errors/walletErrors.js';
import { useCurrentWallet } from './useCurrentWallet.js';
import { useWalletStore } from './useWalletStore.js';
import type {
    StandardConnectInput,
    StandardConnectOutput,
    StandardReconnectForceFeature,
    WalletWithRequiredFeatures,
} from '@mysten/wallet-standard';
import { isSupportedChain } from '@mysten/wallet-standard';
import { getSelectedAccount } from '../../utils/getSelectedAccount.js';

type UseReconnectForceError = WalletNotConnectedError | Error;

type ReconnectForceWalletArgs = {
    /** An optional account address to connect to. Defaults to the first authorized account. */
    accountAddress?: string;
} & StandardConnectInput;

type ReconnectForceWalletResult = StandardConnectOutput;

type UseReconnectForceWalletMutationOptions = Omit<
    UseMutationOptions<
        ReconnectForceWalletResult,
        UseReconnectForceError,
        ReconnectForceWalletArgs,
        unknown
    >,
    'mutationFn'
>;

/**
 * Mutation hook for disconnecting from an active wallet connection, if currently connected.
 */
export function useReconnectForceWallet({
    mutationKey,
    ...mutationOptions
}: UseReconnectForceWalletMutationOptions = {}): UseMutationResult<
    StandardConnectOutput,
    WalletNotConnectedError | Error,
    ReconnectForceWalletArgs,
    unknown
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

                const connectResult = await (
                    currentWallet.features as unknown as WalletWithRequiredFeatures &
                        StandardReconnectForceFeature
                )['sui:reconnectForce']?.reconnect({
                    origin: window.location.origin,
                });
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
