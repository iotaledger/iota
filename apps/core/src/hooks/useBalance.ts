// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

const DEFAULT_REFETCH_INTERVAL = 1000;
const DEFAULT_STALE_TIME = 5000;

export function useBalance(
    address: string | null,
    options: {
        coinType?: string;
        refetchInterval?: number | false;
        staleTime?: number;
    } = {},
) {
    const {
        coinType = IOTA_TYPE_ARG,
        refetchInterval = DEFAULT_REFETCH_INTERVAL,
        staleTime = DEFAULT_STALE_TIME,
    } = options;
    return useIotaClientQuery(
        'getBalance',
        { coinType, owner: address! },
        {
            enabled: !!address,
            refetchInterval,
            staleTime,
        },
    );
}
