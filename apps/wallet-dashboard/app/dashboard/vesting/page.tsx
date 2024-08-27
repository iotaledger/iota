// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Button } from '@/components';
import { useGetCurrentEpochStartTimestamp, useNotifications } from '@/hooks';
import {
    formatDelegatedTimelockedStake,
    getVestingOverview,
    isTimelockedUnlockable,
    mapTimelockObjects,
} from '@/lib/utils';
import { NotificationType } from '@/stores/notificationStore';
import {
    TIMELOCK_IOTA_TYPE,
    useGetAllOwnedObjects,
    useGetStakedTimelockedObjects,
    useUnlockTimelockedObjectsTransaction,
} from '@iota/core';
import {
    useCurrentAccount,
    useIotaClient,
    useSignAndExecuteTransactionBlock,
} from '@iota/dapp-kit';
import { useQueryClient } from '@tanstack/react-query';

function VestingDashboardPage(): JSX.Element {
    const account = useCurrentAccount();
    const queryClient = useQueryClient();
    const iotaClient = useIotaClient();

    const { addNotification } = useNotifications();
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();
    const { data: timelockedObjects } = useGetAllOwnedObjects(account?.address || '', {
        StructType: TIMELOCK_IOTA_TYPE,
    });
    const { data: stakedTimelockedObjects } = useGetStakedTimelockedObjects(account?.address || '');
    const { mutateAsync: signAndExecuteTransactionBlock } = useSignAndExecuteTransactionBlock();

    const timelockedMapped = mapTimelockObjects(timelockedObjects || []);
    const stakedTimelockedMapped = formatDelegatedTimelockedStake(stakedTimelockedObjects || []);

    const vestingSchedule = getVestingOverview(
        [...timelockedMapped, ...stakedTimelockedMapped],
        Number(currentEpochMs),
    );

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
            .waitForTransactionBlock({
                digest,
            })
            .then(() => {
                queryClient.invalidateQueries({
                    queryKey: ['get-staked-timelocked-objects', account?.address],
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
    }

    const handleCollect = () => {
        if (!unlockAllTimelockedObjects?.transactionBlock) {
            addNotification('Failed to create a Transaction', NotificationType.Error);
            return;
        }
        signAndExecuteTransactionBlock(
            {
                transactionBlock: unlockAllTimelockedObjects.transactionBlock,
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
                    {vestingSchedule.availableClaiming ? (
                        <Button onClick={handleCollect}>Collect</Button>
                    ) : null}
                    {vestingSchedule.availableStaking ? (
                        <Button onClick={handleStake}>Stake</Button>
                    ) : null}
                </div>
            )}
        </div>
    );
}

export default VestingDashboardPage;
