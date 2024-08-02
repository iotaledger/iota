// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Button } from '@/components';
import { useGetCurrentEpochStartTimestamp } from '@/hooks';
import { getVestingOverview, mapTimelockObjects } from '@/lib/utils';
import { useCollectUnlockTimelockedObjects, useGetAllTimelockedObjects } from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';

function VestingDashboardPage(): JSX.Element {
    const account = useCurrentAccount();

    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();
    const { data: timelockedObjects } = useGetAllTimelockedObjects(account?.address || '');
    const collect = useCollectUnlockTimelockedObjects(account?.address || '');
    const timelockedMapped = mapTimelockObjects(timelockedObjects || []);
    const vestingSchedule = getVestingOverview(timelockedMapped, Number(currentEpochMs));

    const handleCollect = () => {
        // Update Date.now() when #1217 is merged
        const unlockTimelockedObjects = timelockedMapped?.filter(
            (timelockedObject) => timelockedObject.expirationTimestampMs <= Date.now(),
        );
        console.log('unlockTimelockedObjects', unlockTimelockedObjects);
        collect();
    };
    const handleStake = () => {
        console.log('Stake');
    };

    return (
        <div className="flex flex-col items-center justify-center space-y-4 pt-12">
            <h1>VESTING</h1>
            <div className="flex flex-row space-x-4">
                <div className="flex flex-col items-center rounded-lg border p-4">
                    <span>Total Vested</span>
                    <span>{vestingSchedule.totalVested}</span>
                </div>
                <div className="flex flex-col items-center rounded-lg border p-4">
                    <span>Total Locked</span>
                    <span>{vestingSchedule.totalLocked}</span>
                </div>
                <div className="flex flex-col items-center rounded-lg border p-4">
                    <span>Total Unlocked</span>
                    <span>{vestingSchedule.totalUnlocked}</span>
                </div>
                <div className="flex flex-col items-center rounded-lg border p-4">
                    <span>Total Staked</span>
                    <span>{vestingSchedule.totalStaked}</span>
                </div>
            </div>
            <div className="flex flex-row space-x-4">
                <div className="flex flex-col items-center rounded-lg border p-4">
                    <span>Available Claiming</span>
                    <span>{vestingSchedule.availableClaiming}</span>
                </div>
                <div className="flex flex-col items-center rounded-lg border p-4">
                    <span>Available Staking</span>
                    <span>{vestingSchedule.availableStaking}</span>
                </div>
            </div>
            {account?.address && (
                <div className="flex flex-row space-x-4">
                    {vestingSchedule.availableClaiming && (
                        <Button onClick={handleCollect}>Collect</Button>
                    )}
                    {vestingSchedule.availableStaking && (
                        <Button onClick={handleStake}>Stake</Button>
                    )}
                </div>
            )}
        </div>
    );
}

export default VestingDashboardPage;
