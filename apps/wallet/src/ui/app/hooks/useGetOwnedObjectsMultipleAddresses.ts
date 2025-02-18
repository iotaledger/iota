// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { type PaginatedObjectsResponse, type IotaObjectDataFilter } from '@iota/iota-sdk/client';
import { useInfiniteQuery } from '@tanstack/react-query';

const MAX_OBJECTS_PER_REQ = 6;

export function useGetOwnedObjectsMultipleAddresses(
    addresses: string[],
    filter?: IotaObjectDataFilter,
    maxObjectRequests = MAX_OBJECTS_PER_REQ,
) {
    const client = useIotaClient();

    return useInfiniteQuery<PaginatedObjectsResponse[]>({
        initialPageParam: null,
        queryKey: ['get-owned-objects', addresses, filter, maxObjectRequests],
        queryFn: async ({ pageParam }) => {
            try {
                const responses = await Promise.all(
                    addresses.map((address) =>
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
                    ),
                );
                return responses.flat();
            } catch (error) {
                return [];
            }
        },
        staleTime: 10 * 1000,
        enabled: addresses.length > 0,
        getNextPageParam: (lastPage) => {
            const nextCursor = lastPage.find((res) => res.hasNextPage)?.nextCursor;
            return nextCursor || null;
        },
    });
}
