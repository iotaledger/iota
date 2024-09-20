// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { UseQueryResult } from '@tanstack/react-query';
import { useQueries } from '@tanstack/react-query';

import { useIotaClientContext } from './useIotaClient.js';
import type { IotaRpcMethods, UseIotaClientQueryOptions } from './useIotaClientQuery.js';

type IotaClientQueryOptions = IotaRpcMethods[keyof IotaRpcMethods] extends infer Method
    ? Method extends {
          name: infer M extends keyof IotaRpcMethods;
          params?: infer P;
      }
        ? undefined extends P
            ? {
                  method: M;
                  params?: P;
                  options?: UseIotaClientQueryOptions<M, unknown>;
              }
            : {
                  method: M;
                  params: P;
                  options?: UseIotaClientQueryOptions<M, unknown>;
              }
        : never
    : never;

export type UseIotaClientQueriesResults<Args extends readonly IotaClientQueryOptions[]> = {
    -readonly [K in keyof Args]: Args[K] extends {
        method: infer M extends keyof IotaRpcMethods;
        readonly options?:
            | {
                  select?: (...args: any[]) => infer R;
              }
            | object;
    }
        ? UseQueryResult<unknown extends R ? IotaRpcMethods[M]['result'] : R>
        : never;
};

export function useIotaClientQueries<
    const Queries extends readonly IotaClientQueryOptions[],
    Results = UseIotaClientQueriesResults<Queries>,
>({
    queries,
    combine,
}: {
    queries: Queries;
    combine?: (results: UseIotaClientQueriesResults<Queries>) => Results;
}): Results {
    const iotaContext = useIotaClientContext();

    return useQueries({
        combine: combine as never,
        queries: queries.map((query) => {
            const { method, params, options: { queryKey = [], ...restOptions } = {} } = query;

            return {
                ...restOptions,
                queryKey: [iotaContext.network, method, params, ...queryKey],
                queryFn: async () => {
                    return await iotaContext.client[method](params as never);
                },
            };
        }) as [],
    });
}
