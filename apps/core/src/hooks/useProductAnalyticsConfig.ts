// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';

import { useAppsBackendClient } from '@iota/apps-backend-client';

export function useProductAnalyticsConfig() {
    const client = useAppsBackendClient();
    return useQuery({
        queryKey: ['apps-backend', 'product-analytics-config'],
        queryFn: () => client.getProductAnalyticsConfig(),
        staleTime: 24 * 60 * 60 * 1000,
        gcTime: Infinity,
    });
}
