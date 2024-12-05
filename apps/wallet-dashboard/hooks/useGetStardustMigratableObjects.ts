// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useGetCurrentEpochStartTimestamp } from '@/hooks';
import { groupStardustObjectsByMigrationStatus } from '@/lib/utils';
import {
    STARDUST_BASIC_OUTPUT_TYPE,
    STARDUST_NFT_OUTPUT_TYPE,
    useGetAllOwnedObjects,
} from '@iota/core';
import { IotaObjectData } from '@iota/iota-sdk/client';

export function useGetStardustMigratableObjects(
    address: string,
    stardustEnabled: boolean | null,
): {
    migratableBasicOutputs: IotaObjectData[];
    migratableNftOutputs: IotaObjectData[];
} {
    if (!stardustEnabled) return { migratableBasicOutputs: [], migratableNftOutputs: [] };
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();
    const { data: basicOutputObjects } = useGetAllOwnedObjects(address, {
        StructType: STARDUST_BASIC_OUTPUT_TYPE,
    });
    const { data: nftOutputObjects } = useGetAllOwnedObjects(address, {
        StructType: STARDUST_NFT_OUTPUT_TYPE,
    });
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
