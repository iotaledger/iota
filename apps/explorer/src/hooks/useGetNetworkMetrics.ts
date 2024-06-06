// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export function useGetNetworkMetrics() {
    const client = useIotaClient();
    return useQuery({
        queryKey: ['home', 'metrics'],
        queryFn: () => client.getNetworkMetrics(),
        gcTime: 24 * 60 * 60 * 1000,
        staleTime: Infinity,
        retry: 5,
    });
}
