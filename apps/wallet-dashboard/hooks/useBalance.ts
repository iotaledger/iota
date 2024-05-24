// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useSuiClient } from '@mysten/dapp-kit';
import { CoinBalance } from '@mysten/sui.js/client';
import { MIST_PER_SUI } from '@mysten/sui.js/utils';
import { useQuery, UseQueryOptions } from '@tanstack/react-query';

interface UseBalance extends CoinBalance {
    suiBalance: number;
}

type UseBalanceOptions = {
    coinType: string;
    address?: string;
} & Omit<UseQueryOptions<UseBalance, Error>, 'queryKey' | 'queryFn' | 'enabled'>;

export function useBalance(options: UseBalanceOptions) {
    const client = useSuiClient();
    const { coinType, address, ...queryOptions } = options;

    return useQuery<UseBalance>({
        queryKey: ['get-balance', address, coinType],
        queryFn: async () => {
            const data = await client.getBalance({
                owner: address!,
                coinType,
            });

            return {
                suiBalance: Number(data.totalBalance) / Number(MIST_PER_SUI),
                ...data,
            };
        },
        enabled: !!address,
        ...queryOptions,
    });
}
