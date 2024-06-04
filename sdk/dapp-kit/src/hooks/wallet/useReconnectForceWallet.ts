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
import { isSupportedChain } from '@mysten/wallet-standard';
import type { StandardConnectInput, StandardConnectOutput } from '@mysten/wallet-standard';
import type {
    WalletAccount,
    StandardReconnectForceFeature,
    WalletWithRequiredFeatures,
} from '@mysten/wallet-standard';

type UseDisconnectWalletError = WalletNotConnectedError | Error;

type ConnectWalletArgs = {
    /** An optional account address to connect to. Defaults to the first authorized account. */
    accountAddress?: string;
} & StandardConnectInput;

type ConnectWalletResult = StandardConnectOutput;

type UseDisconnectWalletMutationOptions = Omit<
    UseMutationOptions<ConnectWalletResult, UseDisconnectWalletError, ConnectWalletArgs, unknown>,
    'mutationFn'
>;

/**
 * Mutation hook for disconnecting from an active wallet connection, if currently connected.
 */
export function useReconnectForceWallet({
    mutationKey,
    ...mutationOptions
}: UseDisconnectWalletMutationOptions = {}): UseMutationResult<
    StandardConnectOutput,
    WalletNotConnectedError | Error,
    ConnectWalletArgs,
    unknown
> {
    const { currentWallet } = useCurrentWallet();
    const setWalletConnected = useWalletStore((state) => state.setWalletConnected);

    return useMutation({
        mutationKey: walletMutationKeys.reconnectForceWallet(mutationKey),
        mutationFn: async () => {
            if (!currentWallet) {
                throw new WalletNotConnectedError('No wallet is connected.');
            }

            try {
                const connectResult = await (
                    currentWallet.features as unknown as WalletWithRequiredFeatures &
                        StandardReconnectForceFeature
                )['standard:reconnectForce']?.reconnect({
                    origin: window.location.origin,
                });
                const connectedSuiAccounts = connectResult.accounts.filter((account) =>
                    account.chains.some(isSupportedChain),
                );
                const selectedAccount = getSelectedAccount(connectedSuiAccounts);

                setWalletConnected(currentWallet, connectedSuiAccounts, selectedAccount);

                return { accounts: connectedSuiAccounts };
            } catch (error) {
                throw new WalletNotConnectedError('No wallet is connected.');
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
