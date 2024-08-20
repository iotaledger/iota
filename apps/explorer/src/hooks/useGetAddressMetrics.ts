// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { type AddressMetrics } from '@iota/iota-sdk/src/client';
import { type UseQueryResult, useQuery } from '@tanstack/react-query';

export function useGetAddressMetrics(): UseQueryResult<AddressMetrics, Error> {
    const client = useIotaClient();
    return useQuery<AddressMetrics, Error>({
        queryKey: ['home', 'addresses'],
        queryFn: () => client.getAddressMetrics(),
        gcTime: 24 * 60 * 60 * 1000,
        staleTime: Infinity,
        retry: 5,
    });
}
