// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaClient } from '@iota/iota-sdk/client';
import type {
    UndefinedInitialDataOptions,
    UseQueryOptions,
    UseQueryResult,
} from '@tanstack/react-query';
import { queryOptions, useQuery, useSuspenseQuery } from '@tanstack/react-query';
import { useMemo } from 'react';

import type { PartialBy } from '../types/utilityTypes.js';
import { useIotaClientContext } from './useIotaClient.js';

export type IotaRpcMethodName = {
    [K in keyof IotaClient]: IotaClient[K] extends
        | ((input: any) => Promise<any>)
        | (() => Promise<any>)
        ? K
        : never;
}[keyof IotaClient];

export type IotaRpcMethods = {
    [K in IotaRpcMethodName]: IotaClient[K] extends (input: infer P) => Promise<infer R>
        ? {
              name: K;
              result: R;
              params: P;
          }
        : IotaClient[K] extends () => Promise<infer R>
          ? {
                name: K;
                result: R;
                params: undefined | object;
            }
          : never;
};

export type UseIotaClientQueryOptions<T extends keyof IotaRpcMethods, TData> = PartialBy<
    Omit<UseQueryOptions<IotaRpcMethods[T]['result'], Error, TData, unknown[]>, 'queryFn'>,
    'queryKey'
>;

export type GetIotaClientQueryOptions<T extends keyof IotaRpcMethods> = {
    client: IotaClient;
    network: string;
    method: T;
    options?: PartialBy<
        Omit<UndefinedInitialDataOptions<IotaRpcMethods[T]['result']>, 'queryFn'>,
        'queryKey'
    >;
} & (undefined extends IotaRpcMethods[T]['params']
    ? { params?: IotaRpcMethods[T]['params'] }
    : { params: IotaRpcMethods[T]['params'] });

export function getIotaClientQuery<T extends keyof IotaRpcMethods>({
    client,
    network,
    method,
    params,
    options,
}: GetIotaClientQueryOptions<T>) {
    return queryOptions<IotaRpcMethods[T]['result']>({
        ...options,
        queryKey: [network, method, params],
        queryFn: async () => {
            return await client[method](params as never);
        },
    });
}

export function useIotaClientQuery<
    T extends keyof IotaRpcMethods,
    TData = IotaRpcMethods[T]['result'],
>(
    ...args: undefined extends IotaRpcMethods[T]['params']
        ? [
              method: T,
              params?: IotaRpcMethods[T]['params'],
              options?: UseIotaClientQueryOptions<T, TData>,
          ]
        : [
              method: T,
              params: IotaRpcMethods[T]['params'],
              options?: UseIotaClientQueryOptions<T, TData>,
          ]
): UseQueryResult<TData, Error> {
    const [method, params, { queryKey = [], ...options } = {}] = args as [
        method: T,
        params?: IotaRpcMethods[T]['params'],
        options?: UseIotaClientQueryOptions<T, TData>,
    ];

    const iotaContext = useIotaClientContext();

    return useQuery({
        ...options,
        queryKey: [iotaContext.network, method, params, ...queryKey],
        queryFn: async () => {
            return (await iotaContext.client[method](
                params as never,
            )) as IotaRpcMethods[T]['result'];
        },
    });
}

export function useIotaClientSuspenseQuery<
    T extends keyof IotaRpcMethods,
    TData = IotaRpcMethods[T]['result'],
>(
    ...args: undefined extends IotaRpcMethods[T]['params']
        ? [
              method: T,
              params?: IotaRpcMethods[T]['params'],
              options?: UndefinedInitialDataOptions<TData>,
          ]
        : [
              method: T,
              params: IotaRpcMethods[T]['params'],
              options?: UndefinedInitialDataOptions<TData>,
          ]
) {
    const [method, params, options = {}] = args as [
        method: T,
        params?: IotaRpcMethods[T]['params'],
        options?: UndefinedInitialDataOptions<TData>,
    ];

    const iotaContext = useIotaClientContext();

    const query = useMemo(() => {
        return getIotaClientQuery<T>({
            client: iotaContext.client,
            network: iotaContext.network,
            method,
            params,
            options,
        });
    }, [iotaContext.network, method, params, options]);

    return useSuspenseQuery(query);
}
