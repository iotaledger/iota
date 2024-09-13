// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { type AllEpochsAddressMetrics, type IotaClient } from '@iota/iota-sdk/client';
import { type UseQueryResult, useQuery } from '@tanstack/react-query';

export function useGetAllEpochAddressMetrics(
    ...input: Parameters<IotaClient['getAllEpochAddressMetrics']>
): UseQueryResult<AllEpochsAddressMetrics, Error> {
    const client = useIotaClient();
    return useQuery<AllEpochsAddressMetrics, Error>({
        queryKey: ['get', 'all', 'epoch', 'addresses', ...input],
        queryFn: () => client.getAllEpochAddressMetrics(...input),
    });
}
