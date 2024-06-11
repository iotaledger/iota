// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { keepPreviousData, useInfiniteQuery } from '@tanstack/react-query';

import { type PaginatedTransactionResponse, type TransactionFilter } from '@iota/iota.js/client';

export const DEFAULT_TRANSACTIONS_LIMIT = 20;

// Fetch transaction blocks
export function useGetTransactionBlocks(
    filter?: TransactionFilter,
    limit = DEFAULT_TRANSACTIONS_LIMIT,
    refetchInterval?: number,
): ReturnType<typeof useInfiniteQuery> {
    const client = useIotaClient();

    return useInfiniteQuery<PaginatedTransactionResponse>({
        queryKey: ['get-transaction-blocks', filter, limit],
        queryFn: async ({ pageParam }) =>
            await client.queryTransactionBlocks({
                filter,
                cursor: pageParam as string | null,
                order: 'descending',
                limit,
                options: {
                    showEffects: true,
                    showInput: true,
                },
            }),
        initialPageParam: null,
        getNextPageParam: ({ hasNextPage, nextCursor }) => (hasNextPage ? nextCursor : null),
        staleTime: 10 * 1000,
        retry: false,
        placeholderData: keepPreviousData,
        refetchInterval,
    });
}
