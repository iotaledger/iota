// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { VirtualList } from '@/components';
import { useGetStardustMigratableObjects } from '@/hooks';
import { Button } from '@iota/apps-ui-kit';
import { useCurrentAccount, useIotaClientContext } from '@iota/dapp-kit';
import { getNetwork, IotaObjectData } from '@iota/iota-sdk/client';

function MigrationDashboardPage(): JSX.Element {
    const account = useCurrentAccount();
    const address = account?.address || '';
    const { network } = useIotaClientContext();
    const { explorer } = getNetwork(network);

    const {
        migratableBasicOutputs,
        unmigratableBasicOutputs,
        migratableNftOutputs,
        unmigratableNftOutputs,
    } = useGetStardustMigratableObjects(address);

    const hasMigratableObjects =
        migratableBasicOutputs.length > 0 || migratableNftOutputs.length > 0;

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
            <Button text="Migrate" disabled={!hasMigratableObjects} onClick={() => {}} />
        </div>
    );
}

export default MigrationDashboardPage;
