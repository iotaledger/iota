// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import {
    AccountBalance,
    MyCoins,
    TransactionsOverview,
    StakingOverview,
    MigrationOverview,
} from '@/components';
import { useGetCurrentEpochStartTimestamp } from '@/hooks';
import { groupStardustObjectsByMigrationStatus } from '@/lib/utils';
import { useFeature } from '@growthbook/growthbook-react';
import {
    Feature,
    STARDUST_BASIC_OUTPUT_TYPE,
    STARDUST_NFT_OUTPUT_TYPE,
    useGetAllOwnedObjects,
} from '@iota/core';
import { useCurrentAccount, useCurrentWallet } from '@iota/dapp-kit';
import clsx from 'clsx';

function HomeDashboardPage(): JSX.Element {
    const { connectionStatus } = useCurrentWallet();
    const account = useCurrentAccount();

    const address = account?.address || '';
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

    const stardustMigrationEnabled = useFeature<boolean>(Feature.StardustMigration).value;
    const needsMigration =
        (migratableBasicOutputs.length > 0 || migratableNftOutputs.length > 0) &&
        stardustMigrationEnabled;

    return (
        <main className="flex flex-1 flex-col items-center space-y-8 py-md">
            {connectionStatus === 'connected' && account && (
                <>
                    <div
                        className={clsx(
                            'home-page-grid-container h-full w-full',
                            needsMigration && 'with-migration',
                        )}
                    >
                        <div style={{ gridArea: 'balance' }} className="flex grow overflow-hidden">
                            <AccountBalance />
                        </div>
                        <div style={{ gridArea: 'staking' }} className="flex grow overflow-hidden">
                            <StakingOverview />
                        </div>
                        {needsMigration && (
                            <div
                                style={{ gridArea: 'migration' }}
                                className="flex grow overflow-hidden"
                            >
                                <MigrationOverview />
                            </div>
                        )}
                        <div style={{ gridArea: 'coins' }}>
                            <MyCoins />
                        </div>
                        <div style={{ gridArea: 'vesting' }} className="flex grow overflow-hidden">
                            Vesting
                        </div>
                        <div style={{ gridArea: 'activity' }} className="flex grow overflow-hidden">
                            <TransactionsOverview />
                        </div>
                    </div>
                </>
            )}
        </main>
    );
}

export default HomeDashboardPage;
