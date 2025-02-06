// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useInfiniteQuery } from '@tanstack/react-query';
import {
    mapStardustBasicOutputs,
    mapStardustNftOutputs,
    useStardustIndexerClientContext,
} from '@iota/core';

const MAX_OBJECTS_PER_REQ = 6;

export function useGetSharedObjectsMultipleAddresses(
    addresses: string[],
    pageSize: number = MAX_OBJECTS_PER_REQ,
) {
    const { stardustIndexerClient } = useStardustIndexerClientContext();

    return useInfiniteQuery({
        initialPageParam: { page: 1 },
        queryKey: ['get-shared-objects', addresses, pageSize, stardustIndexerClient?.baseUrl],
        queryFn: async ({ pageParam }) => {
            try {
                const responses = await Promise.all(
                    addresses.map(async (address) => {
                        const nftOutputs = await stardustIndexerClient?.getNftResolvedOutputs(
                            address,
                            {
                                page: pageParam?.page,
                                pageSize,
                            },
                        );

                        const basicOutputs = await stardustIndexerClient?.getBasicResolvedOutputs(
                            address,
                            {
                                page: pageParam?.page,
                                pageSize,
                            },
                        );

                        return {
                            address,
                            nftOutputs: nftOutputs ? nftOutputs.map(mapStardustNftOutputs) : [],
                            basicOutputs: basicOutputs
                                ? basicOutputs.map(mapStardustBasicOutputs)
                                : [],
                        };
                    }),
                );

                return responses;
            } catch (error) {
                return [];
            }
        },
        staleTime: 10 * 1000,
        enabled: addresses.length > 0,
        getNextPageParam: (lastPage) => {
            const nextPage = lastPage.find((res) => res.nftOutputs.length === pageSize);
            return nextPage ? { page: lastPage.length + 1 } : null;
        },
    });
}
