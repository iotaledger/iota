// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Button } from '@/components';
import { useGetCurrentEpochStartTimestamp, useNotifications } from '@/hooks';
import {
    ExtendedDelegatedTimelockedStake,
    formatDelegatedTimelockedStake,
    getVestingOverview,
    isTimelockedUnlocked,
    mapTimelockObjects,
} from '@/lib/utils';
import { NotificationType } from '@/stores/notificationStore';
import {
    TIMELOCK_IOTA_TYPE,
    useGetActiveValidatorsInfo,
    useGetAllOwnedObjects,
    useGetTimelockedStakedObjects,
    useUnlockTimelockedObjects,
} from '@iota/core';
import {
    useCurrentAccount,
    useIotaClient,
    useSignAndExecuteTransactionBlock,
} from '@iota/dapp-kit';
import { useQueryClient } from '@tanstack/react-query';

type TimelockedStakedObjectsGrouped = {
    validatorAddress: string;
    startEpoch: string;
    label: string | null | undefined;
    stakes: ExtendedDelegatedTimelockedStake[];
};

function VestingDashboardPage(): JSX.Element {
    const account = useCurrentAccount();
    const queryClient = useQueryClient();
    const iotaClient = useIotaClient();

    const { addNotification } = useNotifications();
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();
    const { data: activeValidators } = useGetActiveValidatorsInfo();
    const { data: timelockedObjects } = useGetAllOwnedObjects(account?.address || '', {
        StructType: TIMELOCK_IOTA_TYPE,
    });
    const { data: timelockedStakedObjects } = useGetTimelockedStakedObjects(account?.address || '');
    const { mutateAsync: signAndExecuteTransactionBlock } = useSignAndExecuteTransactionBlock();

    const timelockedMapped = mapTimelockObjects(timelockedObjects || []);
    const timelockedstakedMapped = formatDelegatedTimelockedStake(timelockedStakedObjects || []);

    const timelockedStakedObjectsGrouped: TimelockedStakedObjectsGrouped[] =
        groupTimelockedStakedObjects(timelockedstakedMapped || []);

    const vestingSchedule = getVestingOverview(
        [...timelockedMapped, ...timelockedstakedMapped],
        Number(currentEpochMs),
    );

    function groupTimelockedStakedObjects(
        extendedDelegatedTimelockedStake: ExtendedDelegatedTimelockedStake[],
    ): TimelockedStakedObjectsGrouped[] {
        const groupedArray: TimelockedStakedObjectsGrouped[] = [];

        extendedDelegatedTimelockedStake.forEach((obj) => {
            let group = groupedArray.find(
                (g) =>
                    g.validatorAddress === obj.validatorAddress &&
                    g.startEpoch === obj.stakeRequestEpoch &&
                    g.label === obj.label,
            );

            if (!group) {
                group = {
                    validatorAddress: obj.validatorAddress,
                    startEpoch: obj.stakeRequestEpoch,
                    label: obj.label,
                    stakes: [],
                };
                groupedArray.push(group);
            }
            group.stakes.push(obj);
        });

        return groupedArray;
    }

    function getValidatorName(validatorAddress: string): string {
        return (
            activeValidators?.find(
                (activeValidator) => activeValidator.iotaAddress === validatorAddress,
            )?.name || validatorAddress
        );
    }

    const unlockedTimelockedObjects = timelockedMapped?.filter((timelockedObject) =>
        isTimelockedUnlocked(timelockedObject, Number(currentEpochMs)),
    );
    const unlockedTimelockedObjectIds: string[] =
        unlockedTimelockedObjects.map((timelocked) => timelocked.id.id) || [];
    const { data: unlockAllTimelockedObjects } = useUnlockTimelockedObjects(
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

    function handleUnstake(delegatedTimelockedStake: TimelockedStakedObjectsGrouped): void {
        // TODO: handle unstake logic
        console.info('delegatedTimelockedStake', delegatedTimelockedStake);
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
                    {timelockedStakedObjectsGrouped?.map((timelockedStakedObject) => {
                        return (
                            <div
                                key={
                                    timelockedStakedObject.validatorAddress +
                                    timelockedStakedObject.startEpoch +
                                    timelockedStakedObject.label
                                }
                                className="flex w-full flex-row items-center justify-center space-x-4"
                            >
                                <span>
                                    Validator:{' '}
                                    {getValidatorName(timelockedStakedObject.validatorAddress)}
                                </span>
                                <span>Start Epoch: {timelockedStakedObject.startEpoch}</span>
                                <span>Stakes: {timelockedStakedObject.stakes.length}</span>

                                <Button onClick={() => handleUnstake(timelockedStakedObject)}>
                                    Unstake
                                </Button>
                            </div>
                        );
                    })}
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
