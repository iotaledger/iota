// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Button, TimelockedUnstakePopup } from '@/components';
import { useGetCurrentEpochStartTimestamp, useNotifications, usePopups } from '@/hooks';
import {
    formatDelegatedTimelockedStake,
    getVestingOverview,
    groupTimelockedStakedObjects,
    isTimelockedUnlockable,
    mapTimelockObjects,
    TimelockedStakedObjectsGrouped,
} from '@/lib/utils';
import { NotificationType } from '@/stores/notificationStore';
import {
    ImageIcon,
    ImageIconSize,
    TIMELOCK_IOTA_TYPE,
    useGetActiveValidatorsInfo,
    useGetAllOwnedObjects,
    useGetTimelockedStakedObjects,
    useUnlockTimelockedObjectsTransaction,
} from '@iota/core';
import { useCurrentAccount, useIotaClient, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import {
    Panel,
    Title,
    DisplayStats,
    Card,
    CardImage,
    CardBody,
    CardAction,
    CardActionType,
    ButtonType,
} from '@iota/apps-ui-kit';
import { IotaValidatorSummary } from '@iota/iota-sdk/client';
import { useQueryClient } from '@tanstack/react-query';
import { useRouter } from 'next/navigation';
import { useEffect } from 'react';

function VestingDashboardPage(): JSX.Element {
    const account = useCurrentAccount();
    const queryClient = useQueryClient();
    const iotaClient = useIotaClient();
    const router = useRouter();
    const { addNotification } = useNotifications();
    const { openPopup, closePopup } = usePopups();
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
        const validatorInfo = getValidatorByAddress(delegatedTimelockedStake.validatorAddress);
        if (!account || !validatorInfo) {
            addNotification('Cannot create transaction', NotificationType.Error);
            return;
        }

        openPopup(
            <TimelockedUnstakePopup
                accountAddress={account.address}
                delegatedStake={delegatedTimelockedStake}
                validatorInfo={validatorInfo}
                closePopup={closePopup}
                onSuccess={handleOnSuccess}
            />,
        );
    }

    useEffect(() => {
        if (!supplyIncreaseVestingEnabled) {
            router.push('/');
        }
    }, [router, supplyIncreaseVestingEnabled]);

    return (
        <div className="flex flex-row gap-lg">
            <Panel>
                <Title title="Vesting" />
                <div className="flex flex-col px-lg py-sm">
                    <div className="flex flex-row gap-md">
                        <DisplayStats label="Total Vested" value={vestingSchedule.totalVested} />
                        <DisplayStats label="Total Locked" value={vestingSchedule.totalLocked} />
                    </div>
                    <div className="mt-md flex flex-row gap-md">
                        <DisplayStats
                            label="Available Claiming"
                            value={vestingSchedule.availableClaiming}
                        />
                        <DisplayStats
                            label="Available Staking"
                            value={vestingSchedule.availableStaking}
                        />
                    </div>
                    {/* TODO */}
                    <div>
                        {account?.address && (
                            <div className="flex flex-row space-x-4">
                                {vestingSchedule.availableClaiming ? (
                                    <Button onClick={handleCollect}>Collect</Button>
                                ) : null}
                            </div>
                        )}
                    </div>
                </div>
            </Panel>
            <Panel>
                <Title title="Staked Vesting" />
                <div className="flex flex-col px-lg py-sm">
                    <div className="flex flex-row gap-md">
                        <DisplayStats label="Your stake" value={vestingSchedule.totalStaked} />
                        <DisplayStats
                            label="Total Unlocked"
                            value={vestingSchedule.totalUnlocked}
                        />
                    </div>
                </div>
                <div className="flex flex-col px-lg py-sm">
                    <div className="flex w-full flex-col items-center justify-center space-y-4 pt-4">
                        {timelockedStakedObjectsGrouped?.map((timelockedStakedObject) => {
                            const name =
                                getValidatorByAddress(timelockedStakedObject.validatorAddress)
                                    ?.name || timelockedStakedObject.validatorAddress;
                            return (
                                <Card
                                    key={
                                        timelockedStakedObject.validatorAddress +
                                        timelockedStakedObject.stakeRequestEpoch +
                                        timelockedStakedObject.label
                                    }
                                >
                                    <CardImage>
                                        <ImageIcon
                                            src={null}
                                            label={name}
                                            fallback={name}
                                            size={ImageIconSize.Large}
                                        />
                                    </CardImage>
                                    <CardBody title={name} subtitle={'1000 IOTA'} isTextTruncated />
                                    {/* TODO */}
                                    <CardAction
                                        type={CardActionType.SupportingText}
                                        title="Start Earning"
                                        subtitle={timelockedStakedObject.stakeRequestEpoch}
                                    />
                                    <CardAction
                                        type={CardActionType.Button}
                                        buttonType={ButtonType.Primary}
                                        title="Unstake"
                                        onClick={() => handleUnstake(timelockedStakedObject)}
                                    />
                                </Card>
                            );
                        })}
                    </div>
                </div>
            </Panel>
        </div>
    );
}

export default VestingDashboardPage;
