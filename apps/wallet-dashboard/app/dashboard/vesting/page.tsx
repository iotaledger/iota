// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Button } from '@/components';
import { useGetCurrentEpochStartTimestamp } from '@/hooks';
import { getVestingOverview, mapTimelockObjects } from '@/lib/utils';
import {
    TIMELOCK_IOTA_TYPE,
    useGetActiveValidatorsInfo,
    useGetAllOwnedObjects,
    useGetStakedTimelockedObjects,
} from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { DelegatedTimelockedStake } from '@iota/iota-sdk/client';

function VestingDashboardPage(): JSX.Element {
    const account = useCurrentAccount();
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();
    const { data: activeValidators } = useGetActiveValidatorsInfo();
    const { data: timelockedObjects } = useGetAllOwnedObjects(account?.address || '', {
        StructType: TIMELOCK_IOTA_TYPE,
    });
    const { data: stakedTimelockedObjects } = useGetStakedTimelockedObjects(account?.address || '');

    const timelockedMapped = mapTimelockObjects(timelockedObjects || []);
    const vestingSchedule = getVestingOverview(
        [...timelockedMapped, ...(stakedTimelockedObjects || [])],
        Number(currentEpochMs),
    );

    function getValidatorName(validatorAddress: string): string {
        return (
            activeValidators?.find(
                (activeValidator) => activeValidator.iotaAddress === validatorAddress,
            )?.name || '-'
        );
    }

    function handleUnstake(delegatedTimelocked: DelegatedTimelockedStake): void {
        // TODO: handle unstake logic
        console.info('delegatedTimelocked', delegatedTimelocked);
    }

    return (
        <div className="flex flex-row">
            <div className="flex w-1/2 flex-col items-center justify-center space-y-4 pt-12">
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
                </div>
                <hr />
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
            <div className="flex w-1/2 flex-col items-center justify-center space-y-4 pt-12">
                <h1>Staked Vesting</h1>
                <div className="flex flex-row space-x-4">
                    <div className="flex flex-col items-center rounded-lg border p-4">
                        <span>Your stake</span>
                        <span>{vestingSchedule.totalStaked}</span>
                    </div>
                    <div className="flex flex-col items-center rounded-lg border p-4">
                        <span>Total Unlocked</span>
                        <span>{vestingSchedule.totalUnlocked}</span>
                    </div>
                </div>
                <div className="flex w-full flex-col items-center justify-center space-y-4 pt-4">
                    {stakedTimelockedObjects?.map((stakedTimelockedObject) => {
                        return (
                            <div
                                key={stakedTimelockedObject.stakingPool}
                                className="flex w-full flex-row items-center justify-center space-x-4"
                            >
                                <span>
                                    {getValidatorName(stakedTimelockedObject.validatorAddress)}
                                </span>
                                <span>Stakes: {stakedTimelockedObject.stakes.length}</span>

                                <Button onClick={() => handleUnstake(stakedTimelockedObject)}>
                                    Unstake
                                </Button>
                            </div>
                        );
                    })}
                </div>
            </div>
        </div>
    );
}

export default VestingDashboardPage;
