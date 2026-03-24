// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import {
    AccountBalance,
    MyCoins,
    TransactionsOverview,
    StakingOverview,
    MigrationOverview,
    SupplyIncreaseVestingOverview,
} from '@/components';
import { useFeature } from '@growthbook/growthbook-react';
import { Feature } from '@iota/core';
import { useCurrentAccount, useCurrentWallet } from '@iota/dapp-kit';
import { Button, ButtonType } from '@iota/apps-ui-kit';
import { useRouter } from 'next/navigation';
import { useGetSupplyIncreaseVestingObjects } from '@/hooks';
import { SupplyIncreaseUserType } from '@/lib/interfaces';

function HomeDashboardPage(): JSX.Element {
    const { connectionStatus } = useCurrentWallet();
    const account = useCurrentAccount();
    const router = useRouter();
    const address = account?.address || '';
    const { userType } = useGetSupplyIncreaseVestingObjects(address);

    const stardustMigrationEnabled = useFeature<boolean>(Feature.StardustMigration).value;
    const supplyIncreaseVestingEnabled = useFeature<boolean>(Feature.SupplyIncreaseVesting).value;

    return (
        <main className="flex flex-1 flex-col items-center space-y-8 py-md">
            {connectionStatus === 'connected' && account && (
                <>
                    <div className="home-page-grid-container w-full content-start">
                        <div style={{ gridArea: 'balance' }} className="flex grow overflow-hidden">
                            <AccountBalance />
                        </div>
                        <div style={{ gridArea: 'staking' }} className="flex grow overflow-hidden">
                            <StakingOverview />
                        </div>
                        {stardustMigrationEnabled && <MigrationOverview />}
                        <div style={{ gridArea: 'coins' }} className="flex grow overflow-hidden">
                            <MyCoins />
                        </div>
                        {supplyIncreaseVestingEnabled && (
                            <SupplyIncreaseVestingOverview
                                customButton={
                                    userType === SupplyIncreaseUserType.Staker ? (
                                        <Button
                                            type={ButtonType.Primary}
                                            text="Go to Vesting Page"
                                            onClick={() => router.push('/vesting')}
                                        />
                                    ) : undefined
                                }
                            />
                        )}
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
