// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaClient } from '@iota/iota-sdk/client';
import type {
    InfiniteData,
    UseInfiniteQueryOptions,
    UseInfiniteQueryResult,
} from '@tanstack/react-query';
import { useInfiniteQuery } from '@tanstack/react-query';

import type { PartialBy } from '../types/utilityTypes.js';
import { useIotaClientContext } from './useIotaClient.js';

interface PaginatedResult {
    data?: unknown;
    nextCursor?: unknown;
    hasNextPage: boolean;
}

export type IotaRpcPaginatedMethodName = {
    [K in keyof IotaClient]: IotaClient[K] extends (input: any) => Promise<PaginatedResult>
        ? K
        : never;
}[keyof IotaClient];

export type IotaRpcPaginatedMethods = {
    [K in IotaRpcPaginatedMethodName]: IotaClient[K] extends (
        input: infer Params,
    ) => Promise<
        infer Result extends { hasNextPage?: boolean | null; nextCursor?: infer Cursor | null }
    >
        ? {
              name: K;
              result: Result;
              params: Params;
              cursor: Cursor;
          }
        : never;
};

export type UseIotaClientInfiniteQueryOptions<
    T extends keyof IotaRpcPaginatedMethods,
    TData,
> = PartialBy<
    Omit<
        UseInfiniteQueryOptions<
            IotaRpcPaginatedMethods[T]['result'],
            Error,
            TData,
            IotaRpcPaginatedMethods[T]['result'],
            unknown[]
        >,
        'queryFn' | 'initialPageParam' | 'getNextPageParam'
    >,
    'queryKey'
>;

export function useIotaClientInfiniteQuery<
    T extends keyof IotaRpcPaginatedMethods,
    TData = InfiniteData<IotaRpcPaginatedMethods[T]['result']>,
>(
    method: T,
    params: IotaRpcPaginatedMethods[T]['params'],
    {
        queryKey = [],
        enabled = !!params,
        ...options
    }: UseIotaClientInfiniteQueryOptions<T, TData> = {},
): UseInfiniteQueryResult<TData, Error> {
    const iotaContext = useIotaClientContext();

    return useInfiniteQuery({
        ...options,
        initialPageParam: null,
        queryKey: [iotaContext.network, method, params, ...queryKey],
        enabled,
        queryFn: ({ pageParam }) =>
            iotaContext.client[method]({
                ...(params ?? {}),
                cursor: pageParam,
            } as never),
        getNextPageParam: (lastPage) =>
            lastPage.hasNextPage ? (lastPage.nextCursor ?? null) : null,
    });
}
