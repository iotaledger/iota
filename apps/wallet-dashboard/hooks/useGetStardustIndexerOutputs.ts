// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';
import { useGetCurrentEpochStartTimestamp } from '@/hooks';
import {
    mapStardustBasicOutputs,
    mapStardustNftOutputs,
    PageParams,
    StardustIndexerOutput,
    TimeUnit,
    useStardustIndexerClientContext,
} from '@iota/core';
import { IotaObjectData } from '@iota/iota-sdk/client';

const LIMIT_PER_REQ = 50;

export function useGetStardustIndexerOutputs(address: string) {
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();
    const { stardustIndexerClient } = useStardustIndexerClientContext();

    const fetchPaginatedOutputs = async (
        mapFn: (output: StardustIndexerOutput) => IotaObjectData,
        fetchFn: (address: string, params: PageParams) => Promise<StardustIndexerOutput[]>,
    ) => {
        const allOutputs = [];
        let page = 1;
        let hasMoreData = true;

        try {
            while (hasMoreData) {
                const outputs = await fetchFn(address, { page, pageSize: LIMIT_PER_REQ });

                if (!outputs || outputs.length === 0) {
                    hasMoreData = false;
                } else {
                    allOutputs.push(...outputs);
                    page++;
                }
            }
        } catch (e) {
            console.error(e);
        }

        return allOutputs.map(mapFn);
    };

    return useQuery({
        queryKey: ['stardust-indexer-outputs', address, currentEpochMs, stardustIndexerClient],
        queryFn: async () => {
            if (!stardustIndexerClient) {
                return {
                    basic: [],
                    nfts: [],
                };
            }

            const basicObjects = await fetchPaginatedOutputs(
                mapStardustBasicOutputs,
                stardustIndexerClient.getBasicResolvedOutputs,
            );

            const nftObjects = await fetchPaginatedOutputs(
                mapStardustNftOutputs,
                stardustIndexerClient.getNftResolvedOutputs,
            );

            return {
                basic: basicObjects,
                nfts: nftObjects,
            };
        },
        enabled: !!address && currentEpochMs !== undefined,
        staleTime: TimeUnit.ONE_SECOND * TimeUnit.ONE_MINUTE * 5,
        initialData: {
            basic: [],
            nfts: [],
        },
        placeholderData: {
            basic: [],
            nfts: [],
        },
    });
}
