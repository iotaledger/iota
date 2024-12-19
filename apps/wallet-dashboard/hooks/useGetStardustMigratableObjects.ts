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

export function useGetStardustMigratableObjects(address: string) {
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();
    const { data: basicOutputObjects } = useGetAllOwnedObjects(address, {
        StructType: STARDUST_BASIC_OUTPUT_TYPE,
    });
    const { data: nftOutputObjects } = useGetAllOwnedObjects(address, {
        StructType: STARDUST_NFT_OUTPUT_TYPE,
    });

    return useQuery({
        queryKey: [
            'stardust-migratable-objects',
            address,
            currentEpochMs,
            basicOutputObjects,
            nftOutputObjects,
        ],
        queryFn: () => {
            const epochMs = Number(currentEpochMs) || 0;

            const { migratable: migratableBasicOutputs, timelocked: timelockedBasicOutputs } =
                groupStardustObjectsByMigrationStatus(basicOutputObjects ?? [], epochMs, address);

            const { migratable: migratableNftOutputs, timelocked: timelockedNftOutputs } =
                groupStardustObjectsByMigrationStatus(nftOutputObjects ?? [], epochMs, address);

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
            nftOutputObjects !== undefined,
        staleTime: TimeUnit.ONE_SECOND * TimeUnit.ONE_MINUTE * 5,
        placeholderData: {
            migratableBasicOutputs: [],
            timelockedBasicOutputs: [],
            migratableNftOutputs: [],
            timelockedNftOutputs: [],
        },
    });
}
