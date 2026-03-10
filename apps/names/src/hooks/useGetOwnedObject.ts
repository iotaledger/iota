// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { GetObjectParams } from '@iota/iota-sdk/client';
import { useQuery } from '@tanstack/react-query';

import { queryKey } from './queryKey';

export function useGetObject(name: string, params: GetObjectParams) {
    const client = useIotaClient();
    return useQuery({
        queryKey: [...queryKey.getObject(name), params],
        queryFn: async () => {
            if (!params.id) {
                return;
            }

            const { data: objectResponse } = await client.getObject({
                ...params,
            });

            return objectResponse;
        },
        staleTime: 10 * 1000,
        enabled: !!params.id,
    });
}
