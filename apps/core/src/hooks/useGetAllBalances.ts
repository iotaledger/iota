// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount, useIotaClientQuery } from '@iota/dapp-kit';
import { COINS_QUERY_REFETCH_INTERVAL, COINS_QUERY_STALE_TIME } from '../constants';
import { filterAndSortTokenBalances } from '../utils';

export function useGetAllBalances() {
    const selectedAddress = useCurrentAccount()?.address;
    return useIotaClientQuery(
        'getAllBalances',
        { owner: selectedAddress! },
        {
            enabled: !!selectedAddress,
            refetchInterval: COINS_QUERY_REFETCH_INTERVAL,
            staleTime: COINS_QUERY_STALE_TIME,
            select: filterAndSortTokenBalances,
        },
    );
}
