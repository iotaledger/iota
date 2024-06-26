// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type AddressFromFinder } from '_src/shared/accounts';
import { useBackgroundClient } from './useBackgroundClient';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { GAS_TYPE_ARG } from '../redux/slices/iota-objects/Coin';

export function useAccountsFinder(
    accountGapLimit: number,
    addressGapLimit: number,
    sourceID: string,
) {
    const backgroundClient = useBackgroundClient();
    const queryClient = useQueryClient();
    const accountsQuery = useQuery<AddressFromFinder[]>({
        queryKey: ['accounts-finder2', accountGapLimit],
        async queryFn() {
            const response = await backgroundClient.getLastAccountFinderResults(accountGapLimit);
            return response.payload.results;
        },
        enabled: !!sourceID,
    });

    async function init() {
        await backgroundClient.initAccountsFinder();
        queryClient.invalidateQueries({
            queryKey: ['accounts-finder2'],
        });
    }

    async function searchMore() {
        await backgroundClient.searchAccountsFinder(
            4218,
            GAS_TYPE_ARG,
            sourceID,
            accountGapLimit,
            addressGapLimit,
        );
        queryClient.invalidateQueries({
            queryKey: ['accounts-finder2'],
        });
    }

    return {
        ...accountsQuery,
        init,
        searchMore,
    };
}
