// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { UseMutationOptions, UseMutationResult } from '@tanstack/react-query';
import { useMutation } from '@tanstack/react-query';

import { useIotaClientContext } from './useIotaClient.js';
import type { IotaRpcMethods } from './useIotaClientQuery.js';

export type UseIotaClientMutationOptions<T extends keyof IotaRpcMethods> = Omit<
    UseMutationOptions<IotaRpcMethods[T]['result'], Error, IotaRpcMethods[T]['params'], unknown[]>,
    'mutationFn'
>;

export function useIotaClientMutation<T extends keyof IotaRpcMethods>(
    method: T,
    options: UseIotaClientMutationOptions<T> = {},
): UseMutationResult<IotaRpcMethods[T]['result'], Error, IotaRpcMethods[T]['params'], unknown[]> {
    const iotaContext = useIotaClientContext();

    return useMutation({
        ...options,
        mutationFn: async (params) => {
            return await iotaContext.client[method](params as never);
        },
    });
}
