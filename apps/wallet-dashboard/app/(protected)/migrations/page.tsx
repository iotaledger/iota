// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { VirtualList } from '@/components';
import MigratePopup from '@/components/Popup/Popups/MigratePopup';
import { useGetCurrentEpochStartTimestamp, usePopups } from '@/hooks';
import { groupStardustObjectsByMigrationStatus } from '@/lib/utils';
import { Button } from '@iota/apps-ui-kit';
import { useCurrentAccount, useIotaClient, useIotaClientContext } from '@iota/dapp-kit';
import {
    STARDUST_BASIC_OUTPUT_TYPE,
    STARDUST_NFT_OUTPUT_TYPE,
    useGetAllOwnedObjects,
} from '@iota/core';
import { getNetwork, IotaObjectData } from '@iota/iota-sdk/client';
import { useQueryClient } from '@tanstack/react-query';

function MigrationDashboardPage(): JSX.Element {
    const account = useCurrentAccount();
    const address = account?.address || '';
    const { openPopup, closePopup } = usePopups();
    const queryClient = useQueryClient();
    const iotaClient = useIotaClient();
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

    const hasMigratableObjects =
        migratableBasicOutputs.length > 0 || migratableNftOutputs.length > 0;

    const virtualItem = (asset: IotaObjectData): JSX.Element => (
        <a href={`${explorer}/object/${asset.objectId}`} target="_blank" rel="noreferrer">
            {asset.objectId}
        </a>
    );

    function handleOnSuccess(digest: string): void {
        iotaClient
            .waitForTransaction({
                digest,
            })
            .then(() => {
                queryClient.invalidateQueries({
                    queryKey: [
                        'get-all-owned-objects',
                        address,
                        {
                            StructType: STARDUST_BASIC_OUTPUT_TYPE,
                        },
                    ],
                });
                queryClient.invalidateQueries({
                    queryKey: [
                        'get-all-owned-objects',
                        address,
                        {
                            StructType: STARDUST_NFT_OUTPUT_TYPE,
                        },
                    ],
                });
            });
    }
    function openMigratePopup(): void {
        openPopup(
            <MigratePopup
                basicOutputObjects={migratableBasicOutputs}
                nftOutputObjects={migratableNftOutputs}
                closePopup={closePopup}
                onSuccess={handleOnSuccess}
            />,
        );
    }

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
            <Button text="Migrate" disabled={!hasMigratableObjects} onClick={openMigratePopup} />
        </div>
    );
}

export default MigrationDashboardPage;
