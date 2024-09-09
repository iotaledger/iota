// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { ResolvedNameServiceNames } from '@iota/iota-sdk/client';
import type { UseQueryOptions, UseQueryResult } from '@tanstack/react-query';

import { useIotaClientQuery } from './useIotaClientQuery.js';

export function useResolveIotaNSName(
    address?: string | null,
    options?: Omit<
        UseQueryOptions<ResolvedNameServiceNames, Error, string | null, unknown[]>,
        'queryFn' | 'queryKey' | 'select'
    >,
): UseQueryResult<string | null, Error> {
    return useIotaClientQuery(
        'resolveNameServiceNames',
        {
            address: address!,
            limit: 1,
        },
        {
            ...options,
            refetchOnWindowFocus: false,
            retry: false,
            select: (data) => (data.data.length > 0 ? data.data[0] : null),
            enabled: !!address && options?.enabled !== false,
        },
    );
}
