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
    useGetAllStardustSharedObjects,
} from '@iota/core';

export function useGetStardustMigratableObjects(address: string) {
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();
    const { data: stardustSharedObjectsData, isPending: stardustSharedObjectsPending } =
        useGetAllStardustSharedObjects(address);
    const { data: basicOutputObjects } = useGetAllOwnedObjects(address, {
        StructType: STARDUST_BASIC_OUTPUT_TYPE,
    });
    const { data: nftOutputObjects } = useGetAllOwnedObjects(address, {
        StructType: STARDUST_NFT_OUTPUT_TYPE,
    });

    const sharedBasicOutputObjects = stardustSharedObjectsData?.basic ?? [];
    const sharedNftOutputObjects = stardustSharedObjectsData?.nfts ?? [];

    return useQuery({
        queryKey: [
            'stardust-migratable-objects',
            address,
            currentEpochMs,
            basicOutputObjects,
            nftOutputObjects,
            sharedBasicOutputObjects,
            sharedNftOutputObjects,
        ],
        queryFn: () => {
            const epochMs = Number(currentEpochMs) || 0;

            const { migratable: migratableBasicOutputs, timelocked: timelockedBasicOutputs } =
                groupStardustObjectsByMigrationStatus(
                    [...(basicOutputObjects ?? []), ...sharedBasicOutputObjects],
                    epochMs,
                    address,
                );

            const { migratable: migratableNftOutputs, timelocked: timelockedNftOutputs } =
                groupStardustObjectsByMigrationStatus(
                    [...(nftOutputObjects ?? []), ...sharedNftOutputObjects],
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
            !stardustSharedObjectsPending,
        staleTime: TimeUnit.ONE_SECOND * TimeUnit.ONE_MINUTE * 5,
        placeholderData: {
            migratableBasicOutputs: [],
            timelockedBasicOutputs: [],
            migratableNftOutputs: [],
            timelockedNftOutputs: [],
        },
    });
}
