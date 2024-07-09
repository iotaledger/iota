// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type AddressFromFinder } from '_src/shared/accounts';
import { useBackgroundClient } from './useBackgroundClient';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { IOTA_COIN_TYPE_ID, GAS_TYPE_ARG } from '../redux/slices/iota-objects/Coin';
import { type GetAccountsFinderResultsResponse } from '_src/shared/messaging/messages/payloads/accounts-finder';

export interface UseAccountFinderOptions {
    coinType?: number;
    gasType?: string;
    accountGapLimit: number;
    addressGapLimit: number;
    sourceID: string;
}

export function useAccountsFinder({
    coinType = IOTA_COIN_TYPE_ID,
    gasType = GAS_TYPE_ARG,
    addressGapLimit,
    accountGapLimit,
    sourceID,
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

    async function init() {
        await backgroundClient.initAccountsFinder();
        queryClient.invalidateQueries({
            queryKey: ['accounts-finder-results'],
        });
    }

    async function searchMore() {
        await backgroundClient.searchAccountsFinder(
            coinType,
            gasType,
            sourceID,
            accountGapLimit,
            addressGapLimit,
        );
        queryClient.invalidateQueries({
            queryKey: ['accounts-finder-results'],
        });
    }

    return {
        ...accountsQuery,
        init,
        searchMore,
    };
}
