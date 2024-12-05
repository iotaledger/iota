// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useGetCurrentEpochStartTimestamp } from '@/hooks';
import { groupStardustObjectsByMigrationStatus } from '@/lib/utils';
import { useFeature } from '@growthbook/growthbook-react';
import {
    Feature,
    STARDUST_BASIC_OUTPUT_TYPE,
    STARDUST_NFT_OUTPUT_TYPE,
    useGetAllOwnedObjects,
} from '@iota/core';
import { IotaObjectData } from '@iota/iota-sdk/client';

export function useGetStardustMigratableObjects(address: string): {
    migratableBasicOutputs: IotaObjectData[];
    migratableNftOutputs: IotaObjectData[];
} {
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();
    const { data: basicOutputObjects } = useGetAllOwnedObjects(address, {
        StructType: STARDUST_BASIC_OUTPUT_TYPE,
    });
    const { data: nftOutputObjects } = useGetAllOwnedObjects(address, {
        StructType: STARDUST_NFT_OUTPUT_TYPE,
    });

    const stardustMigrationEnabled = useFeature<boolean>(Feature.StardustMigration).value;
    if (!stardustMigrationEnabled) {
        return { migratableBasicOutputs: [], migratableNftOutputs: [] };
    } else {
        const { migratable: migratableBasicOutputs } = groupStardustObjectsByMigrationStatus(
            basicOutputObjects ?? [],
            Number(currentEpochMs),
            address,
        );

        const { migratable: migratableNftOutputs } = groupStardustObjectsByMigrationStatus(
            nftOutputObjects ?? [],
            Number(currentEpochMs),
            address,
        );

        return { migratableBasicOutputs, migratableNftOutputs };
    }
}
