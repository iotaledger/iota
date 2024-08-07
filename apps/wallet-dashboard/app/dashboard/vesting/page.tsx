// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Button } from '@/components';
import { useGetCurrentEpochStartTimestamp, useNotifications } from '@/hooks';
import { getVestingOverview, mapTimelockObjects } from '@/lib/utils';
import { NotificationType } from '@/stores/notificationStore';
import { useCollectUnlockTimelockedObjects, useGetAllTimelockedObjects } from '@iota/core';
import { useCurrentAccount, useSignAndExecuteTransactionBlock } from '@iota/dapp-kit';

function VestingDashboardPage(): JSX.Element {
    const account = useCurrentAccount();

    const { addNotification } = useNotifications();
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();
    const { data: timelockedObjects } = useGetAllTimelockedObjects(account?.address || '');
    const { mutateAsync: signAndExecuteTransactionBlock } = useSignAndExecuteTransactionBlock();

    const timelockedMapped = timelockedObjects ? mapTimelockObjects(timelockedObjects) : [];
    const vestingSchedule = getVestingOverview(timelockedMapped, Number(currentEpochMs));

    const unlockTimelockedObjects = timelockedMapped?.filter(
        (timelockedObject) => timelockedObject.expirationTimestampMs <= Number(currentEpochMs),
    );
    const unlockTimelockedbjectIds: string[] =
        unlockTimelockedObjects.map((timelocked) => timelocked.id.id) || [];
    const { data: unlockAllTimelockedObjects } = useCollectUnlockTimelockedObjects(
        account?.address || '',
        unlockTimelockedbjectIds,
    );

    const handleCollect = () => {
        if (!unlockAllTimelockedObjects?.transaction) {
            addNotification('There was an error with the transaction', NotificationType.Error);
            return;
        } else {
            signAndExecuteTransactionBlock({
                transactionBlock: unlockAllTimelockedObjects.transaction,
            })
                .then(() => {
                    addNotification('Transfer transaction has been sent');
                })
                .catch(() => {
                    addNotification('Transfer transaction was not sent', NotificationType.Error);
                });
        }
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
