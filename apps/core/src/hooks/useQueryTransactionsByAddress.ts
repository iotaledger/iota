// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { useInfiniteQuery } from '@tanstack/react-query';

const QUERY_OPTIONS = {
    showInput: true,
    showEffects: true,
    showEvents: true,
    showBalanceChanges: true,
    showObjectChanges: true,
};

const MAX_OBJECTS_PER_REQ = 20;

interface NextCursor {
    nextCursorToAddress?: string | null;
    nextCursorFromAddress?: string | null;
}

interface FetchTxsResponse extends NextCursor {
    transactions: IotaTransactionBlockResponse[];
    hasNextPage: boolean;
}

export function useQueryTransactionsByAddress(address: string = '') {
    const rpc = useIotaClient();

    const query = useInfiniteQuery<FetchTxsResponse>({
        initialPageParam: null,
        queryKey: ['transactions-by-address', address, QUERY_OPTIONS],
        queryFn: async ({ pageParam }): Promise<FetchTxsResponse> => {
            const [senderResponse, receiverResponse] = await Promise.all([
                rpc.queryTransactionBlocks({
                    options: QUERY_OPTIONS,
                    filter: { ToAddress: address },
                    limit: MAX_OBJECTS_PER_REQ,
                    cursor: (pageParam as NextCursor)?.nextCursorToAddress,
                }),
                rpc.queryTransactionBlocks({
                    options: QUERY_OPTIONS,
                    filter: { FromAddress: address },
                    limit: MAX_OBJECTS_PER_REQ,
                    cursor: (pageParam as NextCursor)?.nextCursorFromAddress,
                }),
            ]);

            const transactions = [...senderResponse.data, ...receiverResponse.data];

            return {
                transactions,
                hasNextPage: senderResponse.hasNextPage || receiverResponse.hasNextPage,
                nextCursorToAddress: senderResponse.nextCursor,
                nextCursorFromAddress: receiverResponse.nextCursor,
            };
        },
        enabled: !!address,
        staleTime: 10 * 1000,
        getNextPageParam: (lastPage) =>
            lastPage.hasNextPage
                ? {
                      nextCursorToAddress: lastPage.nextCursorToAddress,
                      nextCursorFromAddress: lastPage.nextCursorFromAddress,
                  }
                : undefined,
    });
    const flattenTransactions = query.data?.pages.flatMap((page) => page.transactions) || [];
    const allTransactions = Array.from(
        flattenTransactions
            .reduce((map, item) => {
                if (!map.has(item.digest)) {
                    map.set(item.digest, item);
                }
                return map;
            }, new Map())
            .values(),
    );
    const lastPage = query.data?.pages[query.data.pages.length - 1];

    return {
        ...query,
        hasNextPage: lastPage?.hasNextPage || false,
        allTransactions,
    };
}
