// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { type IotaClient } from '@iota/iota.js/client';
import { useQuery } from '@tanstack/react-query';

export function useGetAllEpochAddressMetrics(
    ...input: Parameters<IotaClient['getAllEpochAddressMetrics']>
): ReturnType<typeof useQuery> {
    const client = useIotaClient();
    return useQuery({
        queryKey: ['get', 'all', 'epoch', 'addresses', ...input],
        queryFn: () => client.getAllEpochAddressMetrics(...input),
    });
}
