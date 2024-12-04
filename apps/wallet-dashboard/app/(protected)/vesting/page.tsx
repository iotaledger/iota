// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Banner, StakeDialog, TimelockedUnstakePopup } from '@/components';
import { useStakeDialog } from '@/components/Dialogs/Staking/hooks/useStakeDialog';
import { useGetCurrentEpochStartTimestamp, useNotifications, usePopups } from '@/hooks';
import {
    formatDelegatedTimelockedStake,
    getLatestOrEarliestSupplyIncreaseVestingPayout,
    getVestingOverview,
    groupTimelockedStakedObjects,
    isTimelockedUnlockable,
    mapTimelockObjects,
    TimelockedStakedObjectsGrouped,
} from '@/lib/utils';
import { NotificationType } from '@/stores/notificationStore';
import { useFeature } from '@growthbook/growthbook-react';
import {
    Panel,
    Title,
    TitleSize,
    DisplayStats,
    TooltipPosition,
    Card,
    CardImage,
    CardAction,
    CardActionType,
    CardBody,
    CardType,
    ImageType,
    ImageShape,
    ButtonType,
    Button,
} from '@iota/apps-ui-kit';
import {
    Theme,
    TIMELOCK_IOTA_TYPE,
    useFormatCoin,
    useGetActiveValidatorsInfo,
    useGetAllOwnedObjects,
    useGetTimelockedStakedObjects,
    useTheme,
    useUnlockTimelockedObjectsTransaction,
    useCountdownByTimestamp,
    Feature,
} from '@iota/core';
import { useCurrentAccount, useIotaClient, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { IotaValidatorSummary } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Calendar, StarHex } from '@iota/ui-icons';
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
    const { theme } = useTheme();

    const videoSrc =
        theme === Theme.Dark
            ? 'https://files.iota.org/media/tooling/wallet-dashboard-staking-dark.mp4'
            : 'https://files.iota.org/media/tooling/wallet-dashboard-staking-light.mp4';

    const supplyIncreaseVestingEnabled = useFeature<boolean>(Feature.SupplyIncreaseVesting).value;

    const timelockedMapped = mapTimelockObjects(timelockedObjects || []);
    const timelockedstakedMapped = formatDelegatedTimelockedStake(timelockedStakedObjects || []);

    const timelockedStakedObjectsGrouped: TimelockedStakedObjectsGrouped[] =
        groupTimelockedStakedObjects(timelockedstakedMapped || []);

    const vestingSchedule = getVestingOverview(
        [...timelockedMapped, ...timelockedstakedMapped],
        Number(currentEpochMs),
    );

    const {
        isDialogStakeOpen,
        stakeDialogView,
        setStakeDialogView,
        selectedStake,
        selectedValidator,
        setSelectedValidator,
        handleCloseStakeDialog,
        handleNewStake,
    } = useStakeDialog();

    const nextPayout = getLatestOrEarliestSupplyIncreaseVestingPayout(
        [...timelockedMapped, ...timelockedstakedMapped],
        false,
    );

    const formattedLastPayoutExpirationTime = useCountdownByTimestamp(
        Number(nextPayout?.expirationTimestampMs),
    );

    const [formattedTotalVested, vestedSymbol] = useFormatCoin(
        vestingSchedule.totalVested,
        IOTA_TYPE_ARG,
    );

    const [formattedTotalLocked, lockedSymbol] = useFormatCoin(
        vestingSchedule.totalLocked,
        IOTA_TYPE_ARG,
    );

    const [formattedAvailableClaiming, availableClaimingSymbol] = useFormatCoin(
        vestingSchedule.availableClaiming,
        IOTA_TYPE_ARG,
    );

    const [formattedNextPayout, nextPayoutSymbol] = useFormatCoin(
        nextPayout?.amount,
        IOTA_TYPE_ARG,
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
        <div className="flex w-full max-w-xl flex-col gap-lg justify-self-center">
            <Panel>
                <Title title="Vesting" size={TitleSize.Medium} />
                <div className="flex flex-col gap-md p-lg pt-sm">
                    <div className="flex h-24 flex-row gap-4">
                        <DisplayStats
                            label="Total Vested"
                            value={formattedTotalVested}
                            supportingLabel={vestedSymbol}
                        />
                        <DisplayStats
                            label="Total Locked"
                            value={formattedTotalLocked}
                            supportingLabel={lockedSymbol}
                            tooltipText="Total amount of IOTA that is still locked in your account."
                            tooltipPosition={TooltipPosition.Right}
                        />
                    </div>
                    <Card type={CardType.Outlined}>
                        <CardImage type={ImageType.BgSolid} shape={ImageShape.SquareRounded}>
                            <StarHex className="h-5 w-5 text-primary-30 dark:text-primary-80" />
                        </CardImage>
                        <CardBody
                            title={`${formattedAvailableClaiming} ${availableClaimingSymbol}`}
                            subtitle="Available Rewards"
                        />
                        <CardAction
                            type={CardActionType.Button}
                            onClick={handleCollect}
                            title="Collect"
                            buttonType={ButtonType.Primary}
                            buttonDisabled={
                                !vestingSchedule.availableClaiming ||
                                vestingSchedule.availableClaiming === 0
                            }
                        />
                    </Card>
                    <Card type={CardType.Outlined}>
                        <CardImage type={ImageType.BgSolid} shape={ImageShape.SquareRounded}>
                            <Calendar className="h-5 w-5 text-primary-30 dark:text-primary-80" />
                        </CardImage>
                        <CardBody
                            title={`${formattedNextPayout} ${nextPayoutSymbol}`}
                            subtitle={`Next payout ${
                                nextPayout?.expirationTimestampMs
                                    ? formattedLastPayoutExpirationTime
                                    : ''
                            }`}
                        />
                        <CardAction
                            type={CardActionType.Button}
                            onClick={() => {
                                /*Open schedule dialog*/
                            }}
                            title="See All"
                            buttonType={ButtonType.Secondary}
                            buttonDisabled={
                                !vestingSchedule.availableStaking ||
                                vestingSchedule.availableStaking === 0
                            }
                        />
                    </Card>
                </div>
            </Panel>
            {timelockedstakedMapped.length === 0 ? (
                <>
                    <Banner
                        videoSrc={videoSrc}
                        title="Stake Vested Tokens"
                        subtitle="Earn Rewards"
                        onButtonClick={() => {
                            /*Add stake vested tokens dialog flow*/
                        }}
                        buttonText="Stake"
                    />
                </>
            ) : (
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

                                    <Button
                                        onClick={() => handleUnstake(timelockedStakedObject)}
                                        text="Unstake"
                                    />
                                </div>
                            );
                        })}
                    </div>
                </div>
            )}
            <StakeDialog
                isTimelockedStaking={true}
                stakedDetails={selectedStake}
                onSuccess={handleOnSuccess}
                isOpen={isDialogStakeOpen}
                handleClose={handleCloseStakeDialog}
                view={stakeDialogView}
                setView={setStakeDialogView}
                selectedValidator={selectedValidator}
                setSelectedValidator={setSelectedValidator}
                maxStakableTimelockedAmount={BigInt(vestingSchedule.availableStaking)}
            />
        </div>
    );
}

export default VestingDashboardPage;
