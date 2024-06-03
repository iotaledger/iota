// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { UseQueryResult } from '@tanstack/react-query';
import { useQuery } from '@tanstack/react-query';
// import type {
//     IdentifierRecord,
//     StandardConnectFeature,
//     StandardDisconnectFeature,
//     StandardEventsFeature,
//     SuiSignAndExecuteTransactionBlockFeature,
//     SuiSignMessageFeature,
//     SuiSignPersonalMessageFeature,
//     SuiSignTransactionBlockFeature,
//     Wallet,
// } from '@mysten/wallet-standard';

// import { WalletNotConnectedError } from '../../errors/walletErrors.js';
import { useCurrentWallet } from './useCurrentWallet.js';

export function useAccountList(
    accountAddress: string | undefined,
): UseQueryResult<string[], Error> {
    const { currentWallet } = useCurrentWallet();
    // console.log('--- accountAddress from useAccountList', accountAddress);

    // eslint-disable-next-line @typescript-eslint/ban-ts-comment
    // @ts-ignore
    return useQuery({
        queryKey: ['account-list', accountAddress || 'placeholder-account-list'],
        queryFn: async () => {
            try {
                // eslint-disable-next-line @typescript-eslint/ban-ts-comment
                // @ts-ignore
                const e = await currentWallet.features['standard:accountList']?.get();
                // console.log('--- features standard:accountList response', e);
                return e;
            } catch (error) {
                console.error(
                    'Failed to disconnect the application from the current wallet.',
                    error,
                );
            }
            return ['fff'];
        },
        enabled: !!accountAddress,
    });
}
