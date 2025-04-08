// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaConnect, isSupportedChain, StandardConnect } from '@iota/wallet-standard';
import type {
    IotaConnectMethodInputs,
    StandardConnectOutput,
    WalletAccount,
    WalletWithRequiredFeatures,
} from '@iota/wallet-standard';
import type { UseMutationOptions, UseMutationResult } from '@tanstack/react-query';
import { useMutation } from '@tanstack/react-query';

import { walletMutationKeys } from '../../constants/walletMutationKeys.js';
import { useWalletStore } from './useWalletStore.js';

type ConnectWalletArgs = {
    /** The wallet to connect to. */
    wallet: WalletWithRequiredFeatures;

    /** An optional account address to connect to. Defaults to the first authorized account. */
    accountAddress?: string;
} & IotaConnectMethodInputs;

type ConnectWalletResult = StandardConnectOutput;

type UseConnectWalletMutationOptions = Omit<
    UseMutationOptions<ConnectWalletResult, Error, ConnectWalletArgs, unknown>,
    'mutationFn'
>;

/**
 * Mutation hook for establishing a connection to a specific wallet.
 */
export function useConnectWallet({
    mutationKey,
    ...mutationOptions
}: UseConnectWalletMutationOptions = {}): UseMutationResult<
    ConnectWalletResult,
    Error,
    ConnectWalletArgs,
    unknown
> {
    const setWalletConnected = useWalletStore((state) => state.setWalletConnected);
    const setConnectionStatus = useWalletStore((state) => state.setConnectionStatus);

    return useMutation({
        mutationKey: walletMutationKeys.connectWallet(mutationKey),
        mutationFn: async ({ wallet, accountAddress, ...connectArgs }) => {
            try {
                setConnectionStatus('connecting');

                const standardConnectMethod = wallet.features[StandardConnect].connect;
                const iotaConnectMethod = wallet.features[IotaConnect]?.connect;

                const connectMethod = iotaConnectMethod || standardConnectMethod;

                const connectResult = await connectMethod(connectArgs);
                const connectedIotaAccounts = connectResult.accounts.filter((account) =>
                    account.chains.some(isSupportedChain),
                );
                const selectedAccount = getSelectedAccount(connectedIotaAccounts, accountAddress);

                setWalletConnected(
                    wallet,
                    connectedIotaAccounts,
                    selectedAccount,
                    connectResult.supportedIntents,
                );

                return { accounts: connectedIotaAccounts };
            } catch (error) {
                setConnectionStatus('disconnected');
                throw error;
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
