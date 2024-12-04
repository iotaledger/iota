// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Button, UnstakeTimelockedObjectsDialog } from '@/components';
import { useGetCurrentEpochStartTimestamp, useNotifications } from '@/hooks';
import {
    formatDelegatedTimelockedStake,
    getVestingOverview,
    groupTimelockedStakedObjects,
    isTimelockedUnlockable,
    mapTimelockObjects,
    TimelockedStakedObjectsGrouped,
} from '@/lib/utils';
import { NotificationType } from '@/stores/notificationStore';
import { useFeature } from '@growthbook/growthbook-react';
import {
    Feature,
    TIMELOCK_IOTA_TYPE,
    useGetActiveValidatorsInfo,
    useGetAllOwnedObjects,
    useGetTimelockedStakedObjects,
    useUnlockTimelockedObjectsTransaction,
} from '@iota/core';
import { useCurrentAccount, useIotaClient, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { IotaValidatorSummary } from '@iota/iota-sdk/client';
import { useQueryClient } from '@tanstack/react-query';
import { useRouter } from 'next/navigation';
import { useEffect, useState } from 'react';

export default function VestingDashboardPage(): JSX.Element {
    const [timelockedObjectsToUnstake, setTimelockedObjectsToUnstake] =
        useState<TimelockedStakedObjectsGrouped | null>(null);
    const account = useCurrentAccount();
    const queryClient = useQueryClient();
    const iotaClient = useIotaClient();
    const router = useRouter();
    const { addNotification } = useNotifications();
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();
    const { data: activeValidators } = useGetActiveValidatorsInfo();
    const { data: timelockedObjects } = useGetAllOwnedObjects(account?.address || '', {
        StructType: TIMELOCK_IOTA_TYPE,
    });
    const { data: timelockedStakedObjects } = useGetTimelockedStakedObjects(account?.address || '');
    const { mutateAsync: signAndExecuteTransaction } = useSignAndExecuteTransaction();

    const supplyIncreaseVestingEnabled = useFeature<boolean>(Feature.SupplyIncreaseVesting).value;

    const timelockedMapped = mapTimelockObjects(timelockedObjects || []);
    const timelockedstakedMapped = formatDelegatedTimelockedStake(timelockedStakedObjects || []);

    const timelockedStakedObjectsGrouped: TimelockedStakedObjectsGrouped[] =
        groupTimelockedStakedObjects(timelockedstakedMapped || []);

    const vestingSchedule = getVestingOverview(
        [...timelockedMapped, ...timelockedstakedMapped],
        Number(currentEpochMs),
    );

    function getValidatorByAddress(validatorAddress: string): IotaValidatorSummary | undefined {
        return activeValidators?.find(
            (activeValidator) => activeValidator.iotaAddress === validatorAddress,
        );
    }

    const unlockedTimelockedObjects = timelockedMapped?.filter((timelockedObject) =>
        isTimelockedUnlockable(timelockedObject, Number(currentEpochMs)),
    );
    const unlockedTimelockedObjectIds: string[] =
        unlockedTimelockedObjects.map((timelocked) => timelocked.id.id) || [];
    const { data: unlockAllTimelockedObjects } = useUnlockTimelockedObjectsTransaction(
        account?.address || '',
        unlockedTimelockedObjectIds,
    );

    function handleOnSuccess(digest: string): void {
        iotaClient
            .waitForTransaction({
                digest,
            })
            .then(() => {
                queryClient.invalidateQueries({
                    queryKey: ['get-timelocked-staked-objects', account?.address],
                });
                queryClient.invalidateQueries({
                    queryKey: [
                        'get-all-owned-objects',
                        account?.address,
                        {
                            StructType: TIMELOCK_IOTA_TYPE,
                        },
                    ],
                });
            });
        setTimelockedObjectsToUnstake(null);
    }

    const handleCollect = () => {
        if (!unlockAllTimelockedObjects?.transactionBlock) {
            addNotification('Failed to create a Transaction', NotificationType.Error);
            return;
        }
        signAndExecuteTransaction(
            {
                transaction: unlockAllTimelockedObjects.transactionBlock,
            },
            {
                onSuccess: (tx) => {
                    handleOnSuccess(tx.digest);
                },
            },
        )
            .then(() => {
                addNotification('Collect transaction has been sent');
            })
            .catch(() => {
                addNotification('Collect transaction was not sent', NotificationType.Error);
            });
    };

    function handleUnstake(delegatedTimelockedStake: TimelockedStakedObjectsGrouped): void {
        setTimelockedObjectsToUnstake(delegatedTimelockedStake);
    }

    useEffect(() => {
        if (!supplyIncreaseVestingEnabled) {
            router.push('/');
        }
    }, [router, supplyIncreaseVestingEnabled]);

    return (
        <>
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
                        {timelockedStakedObjectsGrouped?.map((timelockedStakedObject) => {
                            return (
                                <div
                                    key={
                                        timelockedStakedObject.validatorAddress +
                                        timelockedStakedObject.stakeRequestEpoch +
                                        timelockedStakedObject.label
                                    }
                                    className="flex w-full flex-row items-center justify-center space-x-4"
                                >
                                    <span>
                                        Validator:{' '}
                                        {getValidatorByAddress(
                                            timelockedStakedObject.validatorAddress,
                                        )?.name || timelockedStakedObject.validatorAddress}
                                    </span>
                                    <span>
                                        Stake Request Epoch:{' '}
                                        {timelockedStakedObject.stakeRequestEpoch}
                                    </span>
                                    <span>Stakes: {timelockedStakedObject.stakes.length}</span>

                                    <Button onClick={() => handleUnstake(timelockedStakedObject)}>
                                        Unstake
                                    </Button>
                                </div>
                            );
                        })}
                    </div>
                    {account?.address && (
                        <div className="flex flex-row space-x-4">
                            {vestingSchedule.availableClaiming ? (
                                <Button onClick={handleCollect}>Collect</Button>
                            ) : null}
                        </div>
                    )}
                </div>
            </div>

            {timelockedObjectsToUnstake && (
                <UnstakeTimelockedObjectsDialog
                    groupedTimelockedObjects={timelockedObjectsToUnstake}
                    onClose={() => setTimelockedObjectsToUnstake(null)}
                    onSuccess={(tx) => handleOnSuccess(tx.digest)}
                />
            )}
        </>
    );
}
