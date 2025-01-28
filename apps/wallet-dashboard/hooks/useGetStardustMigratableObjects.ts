// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';
import { useGetCurrentEpochStartTimestamp } from '@/hooks';
import { groupStardustObjectsByMigrationStatus } from '@/lib/utils';
import {
    STARDUST_BASIC_OUTPUT_TYPE,
    STARDUST_NFT_OUTPUT_TYPE,
    TimeUnit,
    useGetAllOwnedObjects,
} from '@iota/core';
import { useGetAllStardustSharedObjects } from './useGetAllStardustSharedObjects';

export function useGetStardustMigratableObjects(address: string) {
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();
    const { data: stardustIndexerData, isPending: stardustIndexerPending } =
        useGetAllStardustSharedObjects(address);
    const { data: basicOutputObjects } = useGetAllOwnedObjects(address, {
        StructType: STARDUST_BASIC_OUTPUT_TYPE,
    });
    const { data: nftOutputObjects } = useGetAllOwnedObjects(address, {
        StructType: STARDUST_NFT_OUTPUT_TYPE,
    });

    const stardustIndexerBasics = stardustIndexerData.basic;
    const stardustIndexerNfts = stardustIndexerData.nfts;

    return useQuery({
        queryKey: [
            'stardust-migratable-objects',
            address,
            currentEpochMs,
            basicOutputObjects,
            nftOutputObjects,
            stardustIndexerBasics,
            stardustIndexerNfts,
        ],
        queryFn: () => {
            const epochMs = Number(currentEpochMs) || 0;

            const { migratable: migratableBasicOutputs, timelocked: timelockedBasicOutputs } =
                groupStardustObjectsByMigrationStatus(
                    [...(basicOutputObjects ?? []), ...stardustIndexerBasics],
                    epochMs,
                    address,
                );

            const { migratable: migratableNftOutputs, timelocked: timelockedNftOutputs } =
                groupStardustObjectsByMigrationStatus(
                    [...(nftOutputObjects ?? []), ...stardustIndexerNfts],
                    epochMs,
                    address,
                );

            return {
                migratableBasicOutputs,
                timelockedBasicOutputs,
                migratableNftOutputs,
                timelockedNftOutputs,
            };
        },
        enabled:
            !!address &&
            currentEpochMs !== undefined &&
            basicOutputObjects !== undefined &&
            nftOutputObjects !== undefined &&
            !stardustIndexerPending,
        staleTime: TimeUnit.ONE_SECOND * TimeUnit.ONE_MINUTE * 5,
        placeholderData: {
            migratableBasicOutputs: [],
            timelockedBasicOutputs: [],
            migratableNftOutputs: [],
            timelockedNftOutputs: [],
        },
    });
}
