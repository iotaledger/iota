// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { PaginatedObjectsResponse, type IotaObjectDataFilter } from '@iota/iota.js/client';
import { useInfiniteQuery } from '@tanstack/react-query';
import { TIMELOCK_PACKAGE_ID } from '../constants';

const MAX_OBJECTS_PER_REQ = 6;

export function useGetTimelockedObjects(address: string, maxObjectRequests = MAX_OBJECTS_PER_REQ) {
    const client = useIotaClient();
    const filter: IotaObjectDataFilter = {
        Package: TIMELOCK_PACKAGE_ID,
    };
    return useInfiniteQuery<PaginatedObjectsResponse>({
        initialPageParam: null,
        queryKey: ['get-timelocked-objects', address, filter, maxObjectRequests],
        queryFn: ({ pageParam }) =>
            client.getOwnedObjects({
                owner: address,
                filter,
                options: {
                    showType: true,
                    showContent: true,
                    showDisplay: true,
                },
                limit: maxObjectRequests,
                cursor: pageParam as string | null,
            }),

        staleTime: 10 * 1000,
        enabled: !!address,
        getNextPageParam: ({ hasNextPage, nextCursor }) => (hasNextPage ? nextCursor : null),
    });
}
