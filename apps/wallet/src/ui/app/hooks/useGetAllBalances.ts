// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from '@iota/dapp-kit';
import { useCoinsReFetchingConfig } from './useCoinsReFetchingConfig';
import { filterAndSortTokenBalances } from '@iota/core';
import { useActiveAddress } from './useActiveAddress';

export function useGetAllBalances() {
    const selectedAddress = useActiveAddress();
    const { staleTime, refetchInterval } = useCoinsReFetchingConfig();
    return useIotaClientQuery(
        'getAllBalances',
        { owner: selectedAddress! },
        {
            enabled: !!selectedAddress,
            refetchInterval,
            staleTime,
            select: filterAndSortTokenBalances,
        },
    );
}
