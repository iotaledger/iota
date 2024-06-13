// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota.js/utils';

const DEFAULT_REFETCH_INTERVAL = 1000;
const DEFAULT_STALE_TIME = 5000;

export function useBalance(
    address: string,
    coinType = IOTA_TYPE_ARG,
    refetchInterval: number | false = DEFAULT_REFETCH_INTERVAL,
    staleTime = DEFAULT_STALE_TIME,
) {
    const {
        data: coinBalance,
        isError,
        isPending,
        isFetched,
    } = useIotaClientQuery(
        'getBalance',
        { coinType, owner: address },
        {
            enabled: !!address,
            refetchInterval,
            staleTime,
        },
    );

    return { coinBalance, isPending, isError, isFetched };
}
