// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useGetCurrentEpochStartTimestamp } from '@/hooks';
import {
    getVestingOverview,
    timelockObjectsFromIotaObjects,
    mapTimelockObjects,
} from '@/lib/utils';
import {
    TIMELOCK_IOTA_TYPE,
    useGetAllOwnedObjects,
    useGetStakedTimelockedObjects,
} from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';

function VestingDashboardPage(): JSX.Element {
    const account = useCurrentAccount();
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();
    const { data: timelockedObjects } = useGetAllOwnedObjects(account?.address || '', {
        StructType: TIMELOCK_IOTA_TYPE,
    });
    const { data: stakedTimelockedObjects } = useGetStakedTimelockedObjects(account?.address || '');

    const timelockedMapped = mapTimelockObjects(timelockedObjects || []);
    const stakedTimelockedMapped = timelockObjectsFromIotaObjects(stakedTimelockedObjects || []);
    const vestingSchedule = getVestingOverview(
        [...timelockedMapped, ...stakedTimelockedMapped],
        Number(currentEpochMs),
    );

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
        </div>
    );
}

export default VestingDashboardPage;
