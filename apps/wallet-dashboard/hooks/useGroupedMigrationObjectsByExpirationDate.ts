// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { groupMigrationObjectsByUnlockCondition } from '@/lib/utils';
import { TimeUnit } from '@iota/core';
import { useCurrentAccount, useIotaClient } from '@iota/dapp-kit';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useQuery } from '@tanstack/react-query';

export function useGroupedMigrationObjectsByExpirationDate(
    objects: IotaObjectData[],
    isTimelockUnlockCondition: boolean = false,
) {
    const client = useIotaClient();
    const address = useCurrentAccount()?.address;

    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['grouped-migration-objects', objects, address, isTimelockUnlockCondition],
        queryFn: async () => {
            if (!client || objects.length === 0) {
                return {
                    nftObjects: {},
                    basicObjects: {},
                    nativeTokens: {},
                };
            }
            return await groupMigrationObjectsByUnlockCondition(
                objects,
                client,
                address,
                isTimelockUnlockCondition,
            );
        },
        enabled: !!client && objects.length > 0,
        staleTime: TimeUnit.ONE_SECOND * TimeUnit.ONE_MINUTE * 5,
        select: (data) => ({
            nftObjects: data.nftObjects,
            basicObjects: data.basicObjects,
            nativeTokens: data.nativeTokens,
        }),
    });
}
