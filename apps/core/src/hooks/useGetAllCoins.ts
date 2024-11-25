// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import type { CoinStruct, PaginatedCoins } from '@iota/iota-sdk/client';
import { useQuery } from '@tanstack/react-query';

const MAX_COINS_PER_REQUEST = 100;

// Fetch all coins for an address, this will keep calling the API until all coins are fetched
export function useGetAllCoins(coinType: string, address?: string | null) {
    const rpc = useIotaClient();
    return useQuery({
        queryKey: ['get-all-coins', address, coinType],
        queryFn: async () => {
            let cursor: string | undefined | null = null;
            const allData: CoinStruct[] = [];
            // keep fetching until cursor is null or undefined
            do {
                const { data, nextCursor }: PaginatedCoins = await rpc.getCoins({
                    owner: address!,
                    coinType,
                    cursor,
                    limit: MAX_COINS_PER_REQUEST,
                });
                if (!data || !data.length) {
                    break;
                }

                allData.push(...data);
                cursor = nextCursor;
            } while (cursor);

            return allData;
        },
        enabled: !!address,
    });
}
