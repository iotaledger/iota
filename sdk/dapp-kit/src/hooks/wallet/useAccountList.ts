// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { UseQueryResult } from '@tanstack/react-query';
import { useQuery } from '@tanstack/react-query';

// import { WalletNotConnectedError } from '../../errors/walletErrors.js';
import { useCurrentWallet } from './useCurrentWallet.js';

export function useAccountList(): UseQueryResult<string[], Error> {
    const { currentWallet } = useCurrentWallet();

    // eslint-disable-next-line @typescript-eslint/ban-ts-comment
    // @ts-ignore
    return useQuery({
        queryKey: ['account-list', currentWallet],
        queryFn: async () => {
            try {
                // eslint-disable-next-line @typescript-eslint/ban-ts-comment
                // @ts-ignore
                await currentWallet.features['standard:accountList']?.get();
            } catch (error) {
                console.error(
                    'Failed to disconnect the application from the current wallet.',
                    error,
                );
            }
            return [''];
        },
        enabled: !!currentWallet,
    });
}
