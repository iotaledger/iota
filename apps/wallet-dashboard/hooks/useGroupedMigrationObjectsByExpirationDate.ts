// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ResolvedObjectsGrouped } from '@/lib/types';
import { groupMigrationObjectsByExpirationDate } from '@/lib/utils';
import { useCurrentAccount, useIotaClient } from '@iota/dapp-kit';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { useEffect, useState } from 'react';

export function useGroupedMigrationObjectsByExpirationDate(objects: IotaObjectData[]): {
    isLoading: boolean;
    isErrored: boolean;
    data: ResolvedObjectsGrouped;
} {
    const [isLoading, setIsLoading] = useState(false);
    const [isErrored, setIsErrored] = useState(false);
    const [groupedObjects, setGroupedObjects] = useState({
        nftObjects: {},
        basicObjects: {},
        nativeTokens: {},
    });

    const client = useIotaClient();
    const address = useCurrentAccount()?.address;

    useEffect(() => {
        if (!client || objects.length === 0) {
            setGroupedObjects({
                nftObjects: {},
                basicObjects: {},
                nativeTokens: {},
            });
            setIsErrored(false);
            setIsLoading(false);
            return;
        }

        setIsLoading(true);
        groupMigrationObjectsByExpirationDate(objects, client, address)
            .then((groupedObjects) => {
                setGroupedObjects(groupedObjects);
            })
            .catch((e) => {
                console.error('Error fetching grouped stardust objects:', e);
                setIsErrored(true);
            })
            .finally(() => {
                setIsLoading(false);
            });
    }, [objects, client, address]);

    return {
        isLoading,
        isErrored,
        data: groupedObjects,
    };
}
