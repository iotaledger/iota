// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useSuiClient } from '@mysten/dapp-kit';
import { useQuery } from '@tanstack/react-query';

export function useGetAddressMetrics(): ReturnType<typeof useQuery> {
    const client = useSuiClient();
    return useQuery({
        queryKey: ['home', 'addresses'],
        queryFn: () => client.getAddressMetrics(),
        gcTime: 24 * 60 * 60 * 1000,
        staleTime: Infinity,
        retry: 5,
    });
}
