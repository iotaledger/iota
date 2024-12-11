// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMemo } from 'react';
import { useGetCurrentEpochStartTimestamp } from '@/hooks';
import { groupStardustObjectsByMigrationStatus } from '@/lib/utils';
import {
    STARDUST_BASIC_OUTPUT_TYPE,
    STARDUST_NFT_OUTPUT_TYPE,
    useGetAllOwnedObjects,
} from '@iota/core';

export const useGetStardustMigratableObjects = (address: string) => {
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();
    const { data: basicOutputObjects } = useGetAllOwnedObjects(address, {
        StructType: STARDUST_BASIC_OUTPUT_TYPE,
    });
    const { data: nftOutputObjects } = useGetAllOwnedObjects(address, {
        StructType: STARDUST_NFT_OUTPUT_TYPE,
    });

    return useMemo(() => {
        const epochMs = Number(currentEpochMs) || 0;

        const { migratable: migratableBasicOutputs, unmigratable: unmigratableBasicOutputs } =
            groupStardustObjectsByMigrationStatus(basicOutputObjects ?? [], epochMs, address);

        const { migratable: migratableNftOutputs, unmigratable: unmigratableNftOutputs } =
            groupStardustObjectsByMigrationStatus(nftOutputObjects ?? [], epochMs, address);

        return {
            migratableBasicOutputs,
            unmigratableBasicOutputs,
            migratableNftOutputs,
            unmigratableNftOutputs,
        };
    }, [address, basicOutputObjects, currentEpochMs, nftOutputObjects]);
};
