// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { DynamicFieldPage } from '@iota/iota-sdk/client';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';
import { useInfiniteQuery } from '@tanstack/react-query';

const MAX_PAGE_SIZE = 10;

export function useGetDynamicFields(parentId: string, maxPageSize = MAX_PAGE_SIZE) {
    const client = useIotaClient();
    return useInfiniteQuery<DynamicFieldPage>({
        queryKey: ['dynamic-fields', { maxPageSize, parentId }],
        queryFn: ({ pageParam = null }) =>
            client.getDynamicFields({
                parentId: normalizeIotaAddress(parentId),
                cursor: pageParam as string | null,
                limit: maxPageSize,
            }),
        enabled: !!parentId,
        initialPageParam: null,
        getNextPageParam: ({ nextCursor, hasNextPage }) => (hasNextPage ? nextCursor : null),
    });
}
