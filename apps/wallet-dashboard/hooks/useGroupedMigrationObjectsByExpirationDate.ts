// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ResolvedObjectTypes } from '@/lib/types';
import {
    groupMigrationObjectsByUnlockCondition,
    sortStardustResolvedObjectsByExpiration,
} from '@/lib/utils';
import { TimeUnit } from '@iota/core';
import { useCurrentAccount, useIotaClient } from '@iota/dapp-kit';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useQuery } from '@tanstack/react-query';
import { useGetCurrentEpochStartTimestamp } from './useGetCurrentEpochStartTimestamp';
import { useGetCurrentEpochEndTimestamp } from './useGetCurrentEpochEndTimestamp';

export function useGroupedMigrationObjectsByExpirationDate(
    objects: IotaObjectData[],
    isTimelockUnlockCondition: boolean = false,
) {
    const client = useIotaClient();
    const address = useCurrentAccount()?.address;

    const { data: currentEpochStartTimestampMs } = useGetCurrentEpochStartTimestamp();
    const { data: currentEpochEndTimestampMs } = useGetCurrentEpochEndTimestamp();

    const epochStartMs = currentEpochStartTimestampMs ? parseInt(currentEpochStartTimestampMs) : 0;
    const epochEndMs = currentEpochEndTimestampMs ? currentEpochEndTimestampMs : 0;

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: [
            'grouped-migration-objects',
            objects,
            address,
            isTimelockUnlockCondition,
            epochStartMs,
            epochEndMs,
        ],
        queryFn: async (): Promise<ResolvedObjectTypes[]> => {
            if (!client || objects.length === 0) {
                return [];
            }
            const resolvedObjects = await groupMigrationObjectsByUnlockCondition(
                objects,
                client,
                address,
                isTimelockUnlockCondition,
            );

            return sortStardustResolvedObjectsByExpiration(
                resolvedObjects,
                epochStartMs,
                epochEndMs,
            );
        },
        enabled: !!client && objects.length > 0,
        staleTime: TimeUnit.ONE_SECOND * TimeUnit.ONE_MINUTE * 5,
    });
}
