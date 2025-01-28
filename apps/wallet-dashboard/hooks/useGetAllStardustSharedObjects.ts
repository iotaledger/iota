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

export function useGetAllStardustSharedObjects(address: string) {
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();
    const { stardustIndexerClient } = useStardustIndexerClientContext();

    const fetchPaginatedStardustSharedObjects = async (
        mapFn: (output: StardustIndexerOutput) => IotaObjectData,
        fetchFn: (address: string, params: PageParams) => Promise<StardustIndexerOutput[]>,
    ) => {
        const allData: StardustIndexerOutput[] = [];
        let page = 1;
        const hasMoreData = true;

        try {
            do {
                const data = await fetchFn(address, { page, pageSize: LIMIT_PER_REQ });

                if (!data || !data.length) {
                    break;
                }

                allData.push(...data);
                page++;
            } while (page);
            while (hasMoreData);
        } catch (e) {
            console.error(e);
        }

        return allData.map(mapFn);
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

            const basicOutputs = await fetchPaginatedStardustSharedObjects(
                mapStardustBasicOutputs,
                stardustIndexerClient.getBasicResolvedOutputs,
            );

            const nftOutputs = await fetchPaginatedStardustSharedObjects(
                mapStardustNftOutputs,
                stardustIndexerClient.getNftResolvedOutputs,
            );

            return {
                basic: basicOutputs,
                nfts: nftOutputs,
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
