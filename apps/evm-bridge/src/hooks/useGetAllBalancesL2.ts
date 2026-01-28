// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';
import { useEvmRpcClient } from '../contexts';
import { CoinBalance } from '@iota/iota-sdk/client';
import { AssetsResponse } from '@iota/isc-sdk';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useCoinsReFetchingConfig } from '@iota/core';

export function useGetAllBalancesL2(address: string) {
    const { staleTime, refetchInterval } = useCoinsReFetchingConfig();
    const { evmRpcClient } = useEvmRpcClient();

    return useQuery({
        queryKey: ['anchor-balance-base-token', address, evmRpcClient?.baseUrl],
        queryFn: async () => {
            if (!evmRpcClient?.baseUrl || !address) {
                // Return a properly typed empty AssetsResponse
                return {
                    baseTokens: '0',
                    nativeTokens: [],
                } as AssetsResponse;
            }

            return await evmRpcClient.getBalanceBaseToken(address);
        },
        select: (data: AssetsResponse): CoinBalance[] => {
            if (!data) return [];

            const coinBalances: Array<{
                coinType: string;
                coinObjectCount: number;
                totalBalance: string;
            }> = [];

            if (data.baseTokens) {
                coinBalances.push({
                    coinType: IOTA_TYPE_ARG,
                    coinObjectCount: 1,
                    totalBalance: data.baseTokens,
                });
            }

            for (const { coinType, balance } of data.nativeTokens ?? []) {
                if (coinType && balance) {
                    coinBalances.push({
                        coinType,
                        coinObjectCount: 1,
                        totalBalance: balance,
                    });
                }
            }

            return coinBalances;
        },
        enabled: !!address && !!evmRpcClient?.baseUrl,
        staleTime,
        refetchInterval,
    });
}
