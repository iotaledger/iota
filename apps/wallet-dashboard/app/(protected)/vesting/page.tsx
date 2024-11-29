// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { TimelockedUnstakePopup } from '@/components';
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
import { useFeature } from '@growthbook/growthbook-react';
import {
    Feature,
    ImageIcon,
    ImageIconSize,
    TIMELOCK_IOTA_TYPE,
    useFormatCoin,
    useGetActiveValidatorsInfo,
    useGetAllOwnedObjects,
    useGetTimelockedStakedObjects,
    useUnlockTimelockedObjectsTransaction,
} from '@iota/core';
import { useCurrentAccount, useIotaClient, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { Stake } from '@iota/ui-icons';
import {
    Panel,
    Title,
    DisplayStats,
    Card,
    CardImage,
    CardBody,
    CardAction,
    CardActionType,
    ImageShape,
    CardType,
    ButtonType,
} from '@iota/apps-ui-kit';
import { IotaValidatorSummary } from '@iota/iota-sdk/client';
import { useQueryClient } from '@tanstack/react-query';
import { useRouter } from 'next/navigation';
import { useEffect } from 'react';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

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

    console.log('dd', timelockedStakedObjects);
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
    const [totalVestedFormatted, totalVestedSymbol] = useFormatCoin(
        vestingSchedule.totalVested,
        IOTA_TYPE_ARG,
    );
    const [totalLockedFormatted, totalLockedSymbol] = useFormatCoin(
        vestingSchedule.totalLocked,
        IOTA_TYPE_ARG,
    );
    const [availableClaimingFormatted, availableClaimingSymbol] = useFormatCoin(
        vestingSchedule.availableClaiming,
        IOTA_TYPE_ARG,
    );

    const [availableStakingFormatted, availableStakingSymbol] = useFormatCoin(
        vestingSchedule.availableStaking,
        IOTA_TYPE_ARG,
    );

    const [totalStakedFormatted, totalStakedSymbol] = useFormatCoin(
        vestingSchedule.totalStaked,
        IOTA_TYPE_ARG,
    );

    const [totalUnlockedFormatted, totalUnlockedSymbol] = useFormatCoin(
        vestingSchedule.totalUnlocked,
        IOTA_TYPE_ARG,
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
                <div className="flex flex-col gap-md px-lg py-sm">
                    <div className="flex flex-row gap-md">
                        <DisplayStats
                            label="Total Vested"
                            value={`${totalVestedFormatted} ${totalVestedSymbol}`}
                        />
                        <DisplayStats
                            label="Total Locked"
                            value={`${totalLockedFormatted} ${totalLockedSymbol}`}
                        />
                    </div>
                    <div className="mt-md flex flex-row gap-md">
                        <DisplayStats
                            label="Available Claiming"
                            value={`${availableClaimingFormatted} ${availableClaimingSymbol}`}
                        />
                        <DisplayStats
                            label="Available Staking"
                            value={`${availableStakingFormatted} ${availableStakingSymbol}`}
                        />
                    </div>

                    <div className="">
                        {account?.address && vestingSchedule.availableClaiming ? (
                            <Card type={CardType.Outlined}>
                                <CardImage shape={ImageShape.SquareRounded}>
                                    <Stake />
                                </CardImage>
                                <CardBody
                                    title={`${availableClaimingFormatted} ${availableClaimingSymbol}`}
                                    subtitle={`Available Rewards`}
                                    isTextTruncated
                                />
                                <CardAction
                                    buttonType={ButtonType.Primary}
                                    type={CardActionType.Button}
                                    title="Collect"
                                    onClick={handleCollect}
                                />
                            </Card>
                        ) : null}
                    </div>
                </div>
            </Panel>
            <Panel>
                <Title title="Staked Vesting" />
                <div className="flex flex-col px-lg py-sm">
                    <div className="flex flex-row gap-md">
                        <DisplayStats
                            label="Your stake"
                            value={`${totalStakedFormatted} ${totalStakedSymbol}`}
                        />
                        <DisplayStats
                            label="Total Unlocked"
                            value={`${totalUnlockedFormatted} ${totalUnlockedSymbol}`}
                        />
                    </div>
                </div>
                <div className="flex flex-col px-lg py-sm">
                    <div className="flex w-full flex-col items-center justify-center space-y-4 pt-4">
                        {timelockedStakedObjectsGrouped?.map((timelockedStakedObject) => {
                            return (
                                <TimelockedStakedObject
                                    key={
                                        timelockedStakedObject.validatorAddress +
                                        timelockedStakedObject.stakeRequestEpoch +
                                        timelockedStakedObject.label
                                    }
                                    getValidatorByAddress={getValidatorByAddress}
                                    timelockedStakedObject={timelockedStakedObject}
                                    handleUnstake={handleUnstake}
                                />
                            );
                        })}
                    </div>
                </div>
            </Panel>
        </div>
    );
}

interface TimelockedStakedObjectProps {
    timelockedStakedObject: TimelockedStakedObjectsGrouped;
    handleUnstake: (timelockedStakedObject: TimelockedStakedObjectsGrouped) => void;
    getValidatorByAddress: (validatorAddress: string) => IotaValidatorSummary | undefined;
}
function TimelockedStakedObject({
    getValidatorByAddress,
    timelockedStakedObject,
    handleUnstake,
}: TimelockedStakedObjectProps) {
    const name =
        getValidatorByAddress(timelockedStakedObject.validatorAddress)?.name ||
        timelockedStakedObject.validatorAddress;
    const sum = timelockedStakedObject.stakes.reduce(
        (acc, stake) => {
            const estimatedReward = stake.status === 'Active' ? stake.estimatedReward : 0;

            return {
                principal: Number(stake.principal) + acc.principal,
                estimatedReward: Number(estimatedReward) + acc.estimatedReward,
            };
        },
        {
            principal: 0,
            estimatedReward: 0,
        },
    );

    const [sumPrincipalFormatted, sumPrincipalSymbol] = useFormatCoin(sum.principal, IOTA_TYPE_ARG);
    const [estimatedRewardFormatted, estimatedRewardSymbol] = useFormatCoin(
        sum.estimatedReward,
        IOTA_TYPE_ARG,
    );

    const supportingText = (() => {
        if (timelockedStakedObject.stakes.every((s) => s.status === 'Active')) {
            return {
                title: 'Estimated Reward',
                subtitle: `${estimatedRewardFormatted} ${estimatedRewardSymbol}`,
            };
        }

        return {
            title: 'Stake Request Epoch',
            subtitle: timelockedStakedObject.stakeRequestEpoch,
        };
    })();

    return (
        <Card onClick={() => handleUnstake(timelockedStakedObject)}>
            <CardImage>
                <ImageIcon src={null} label={name} fallback={name} size={ImageIconSize.Large} />
            </CardImage>
            <CardBody
                title={name}
                subtitle={`${sumPrincipalFormatted} ${sumPrincipalSymbol}`}
                isTextTruncated
            />
            <CardAction
                type={CardActionType.SupportingText}
                title={supportingText.title}
                subtitle={supportingText.subtitle}
            />
        </Card>
    );
}

export default VestingDashboardPage;
