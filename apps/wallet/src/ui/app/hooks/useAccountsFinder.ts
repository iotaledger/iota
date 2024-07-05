// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type AddressFromFinder } from '_src/shared/accounts';
import { useBackgroundClient } from './useBackgroundClient';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { IOTA_BIP44_COIN_TYPE, IOTA_GAS_COIN_TYPE } from '../redux/slices/iota-objects/Coin';
import { type GetAccountsFinderResultsResponse } from '_src/shared/messaging/messages/payloads/accounts-finder';
import { type AllowedAccountTypes } from '_src/background/accounts-finder';

export interface UseAccountFinderOptions {
    accountType?: AllowedAccountTypes;
    coinType?: number;
    gasType?: string;
    accountGapLimit?: number;
    addressGapLimit?: number;
    sourceID: string;
}

export function useAccountsFinder({
    coinType = IOTA_BIP44_COIN_TYPE,
    gasType = IOTA_GAS_COIN_TYPE,
    addressGapLimit,
    accountGapLimit,
    sourceID,
    accountType,
}: UseAccountFinderOptions) {
    const backgroundClient = useBackgroundClient();
    const queryClient = useQueryClient();
    const accountsQuery = useQuery<AddressFromFinder[]>({
        queryKey: ['accounts-finder-results'],
        async queryFn() {
            const response = await backgroundClient.getLastAccountFinderResults();
            const payload = response.payload as GetAccountsFinderResultsResponse;
            return payload.results;
        },
        enabled: !!sourceID,
    });

    async function reset() {
        await backgroundClient.resetAccountsFinder();
        queryClient.invalidateQueries({
            queryKey: ['accounts-finder-results'],
        });
    }

    async function searchMore() {
        if (!accountType) return;

        await backgroundClient.searchAccountsFinder({
            accountType,
            bip44CoinType: coinType,
            coinType: gasType,
            sourceID,
            accountGapLimit,
            addressGapLimit,
        });
        queryClient.invalidateQueries({
            queryKey: ['accounts-finder-results'],
        });
    }

    return {
        ...accountsQuery,
        reset: reset,
        searchMore,
    };
}
