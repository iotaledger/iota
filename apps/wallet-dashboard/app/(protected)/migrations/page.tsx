// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { VirtualList } from '@/components';
import { useGetCurrentEpochStartTimestamp } from '@/hooks';
import { groupStardustObjectsByMigrationStatus } from '@/lib/utils';
import {
    STARDUST_BASIC_OUTPUT_TYPE,
    STARDUST_NFT_OUTPUT_TYPE,
    useGetAllOwnedObjects,
} from '@iota/core';
import { useCurrentAccount, useIotaClientContext } from '@iota/dapp-kit';
import { getNetwork, IotaObjectData } from '@iota/iota-sdk/client';

function MigrationDashboardPage(): JSX.Element {
    const account = useCurrentAccount();
    const address = account?.address || '';
    const { network } = useIotaClientContext();
    const { explorer } = getNetwork(network);
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();

    const { data: basicOutputObjects } = useGetAllOwnedObjects(address, {
        StructType: STARDUST_BASIC_OUTPUT_TYPE,
    });
    const { data: nftOutputObjects } = useGetAllOwnedObjects(address, {
        StructType: STARDUST_NFT_OUTPUT_TYPE,
    });

    const { migratable: migratableBasicOutputs, unmigratable: unmigratableBasicOutputs } =
        groupStardustObjectsByMigrationStatus(
            basicOutputObjects ?? [],
            Number(currentEpochMs),
            address,
        );

    const { migratable: migratableNftOutputs, unmigratable: unmigratableNftOutputs } =
        groupStardustObjectsByMigrationStatus(
            nftOutputObjects ?? [],
            Number(currentEpochMs),
            address,
        );
    const virtualItem = (asset: IotaObjectData): JSX.Element => (
        <a href={`${explorer}/object/${asset.objectId}`} target="_blank" rel="noreferrer">
            {asset.objectId}
        </a>
    );

    return (
        <div className="flex h-full w-full flex-wrap items-center justify-center space-y-4">
            <div className="flex w-1/2 flex-col">
                <h1>Migratable Basic Outputs: {migratableBasicOutputs.length}</h1>
                <VirtualList
                    items={migratableBasicOutputs}
                    estimateSize={() => 30}
                    render={virtualItem}
                />
            </div>
            <div className="flex w-1/2 flex-col">
                <h1>Unmigratable Basic Outputs: {unmigratableBasicOutputs.length}</h1>
                <VirtualList
                    items={unmigratableBasicOutputs}
                    estimateSize={() => 30}
                    render={virtualItem}
                />
            </div>
            <div className="flex w-1/2 flex-col">
                <h1>Migratable NFT Outputs: {migratableNftOutputs.length}</h1>
                <VirtualList
                    items={migratableNftOutputs}
                    estimateSize={() => 30}
                    render={virtualItem}
                />
            </div>
            <div className="flex w-1/2 flex-col">
                <h1>Unmigratable NFT Outputs: {unmigratableNftOutputs.length}</h1>
                <VirtualList
                    items={unmigratableNftOutputs}
                    estimateSize={() => 30}
                    render={virtualItem}
                />
            </div>
        </div>
    );
}

export default MigrationDashboardPage;
