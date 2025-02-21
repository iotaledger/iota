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
    uniqueList: IotaTransactionBlockResponse[];
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

            const uniqueList: IotaTransactionBlockResponse[] = [];
            const inserted = new Set();

            [...senderResponse.data, ...receiverResponse.data]
                .sort((a, b) => Number(b.timestampMs ?? 0) - Number(a.timestampMs ?? 0))
                .forEach((txb) => {
                    if (inserted.has(txb.digest)) return;
                    uniqueList.push(txb);
                    inserted.add(txb.digest);
                });

            return {
                uniqueList,
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
    const allTransactions = query.data?.pages.flatMap((page) => page.uniqueList) || [];
    const lastPage = query.data?.pages[query.data.pages.length - 1];

    return {
        ...query,
        hasNextPage: lastPage?.hasNextPage || false,
        allTransactions,
    };
}
