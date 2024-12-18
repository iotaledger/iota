// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { PaginatedCoins } from '@iota/iota-sdk/client';
import { useInfiniteQuery } from '@tanstack/react-query';

const MAX_COINS_PER_REQUEST = 10;

export function useGetCoins(
    coinType: string,
    address?: string | null,
    maxCoinsPerRequest = MAX_COINS_PER_REQUEST,
) {
    const client = useIotaClient();
    return useInfiniteQuery<PaginatedCoins>({
        queryKey: ['get-coins', address, coinType, maxCoinsPerRequest],
        initialPageParam: null,
        getNextPageParam: ({ hasNextPage, nextCursor }) => (hasNextPage ? nextCursor : null),
        queryFn: ({ pageParam }) =>
            client.getCoins({
                owner: address!,
                coinType,
                cursor: pageParam as string | null,
                limit: maxCoinsPerRequest,
            }),
        enabled: !!address,
    });
}
