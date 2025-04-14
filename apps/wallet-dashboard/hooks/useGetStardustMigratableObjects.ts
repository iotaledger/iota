// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetCurrentEpochStartTimestamp } from '@/hooks';
import { groupStardustObjectsByMigrationStatus } from '@/lib/utils';
import {
    STARDUST_BASIC_OUTPUT_TYPE,
    STARDUST_NFT_OUTPUT_TYPE,
    useGetAllOwnedObjects,
    useGetAllStardustSharedObjects,
} from '@iota/core';

export function useGetStardustMigratableObjects(address: string) {
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();
    const stardustSharedObjectsData = useGetAllStardustSharedObjects(address);
    const { data: basicOutputObjects, isPending: isBasicOutputsObjectsPending } =
        useGetAllOwnedObjects(address, {
            StructType: STARDUST_BASIC_OUTPUT_TYPE,
        });
    const { data: nftOutputObjects, isPending: isNftOutputsObjectsPending } = useGetAllOwnedObjects(
        address,
        {
            StructType: STARDUST_NFT_OUTPUT_TYPE,
        },
    );

    const sharedBasicOutputObjects = stardustSharedObjectsData?.basic ?? [];
    const sharedNftOutputObjects = stardustSharedObjectsData?.nfts ?? [];

    const epochMs = currentEpochMs || 0;

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
        isPending: isBasicOutputsObjectsPending || isNftOutputsObjectsPending,
    };
}
