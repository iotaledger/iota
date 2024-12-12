// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import {
    Banner,
    StakeDialog,
    StakeDialogView,
    TimelockedUnstakePopup,
    useStakeDialog,
    VestingScheduleDialog,
} from '@/components';
import { useGetSupplyIncreaseVestingObjects, useNotifications, usePopups } from '@/hooks';
import { groupTimelockedStakedObjects, TimelockedStakedObjectsGrouped } from '@/lib/utils';
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
    Button,
    ButtonType,
    LoadingIndicator,
} from '@iota/apps-ui-kit';
import {
    Theme,
    TIMELOCK_IOTA_TYPE,
    useFormatCoin,
    useGetActiveValidatorsInfo,
    useTheme,
    useCountdownByTimestamp,
    Feature,
} from '@iota/core';
import {
    useCurrentAccount,
    useIotaClient,
    useIotaClientQuery,
    useSignAndExecuteTransaction,
} from '@iota/dapp-kit';
import { IotaValidatorSummary } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Calendar, StarHex } from '@iota/ui-icons';
import { useQueryClient } from '@tanstack/react-query';
import { useRouter } from 'next/navigation';
import { useEffect, useState } from 'react';
import { StakedTimelockObject } from '@/components';

function VestingDashboardPage(): JSX.Element {
    const account = useCurrentAccount();
    const address = account?.address || '';
    const queryClient = useQueryClient();
    const iotaClient = useIotaClient();
    const router = useRouter();
    const { data: system } = useIotaClientQuery('getLatestIotaSystemState');
    const [isVestingScheduleDialogOpen, setIsVestingScheduleDialogOpen] = useState(false);
    const { addNotification } = useNotifications();
    const { openPopup, closePopup } = usePopups();
    const { data: activeValidators } = useGetActiveValidatorsInfo();
    const { mutateAsync: signAndExecuteTransaction } = useSignAndExecuteTransaction();
    const { theme } = useTheme();

    const videoSrc =
        theme === Theme.Dark
            ? 'https://files.iota.org/media/tooling/wallet-dashboard-staking-dark.mp4'
            : 'https://files.iota.org/media/tooling/wallet-dashboard-staking-light.mp4';

    const supplyIncreaseVestingEnabled = useFeature<boolean>(Feature.SupplyIncreaseVesting).value;

    const {
        nextPayout,
        supplyIncreaseVestingPortfolio,
        supplyIncreaseVestingSchedule,
        supplyIncreaseVestingMapped,
        supplyIncreaseVestingStakedMapped,
        istimelockedStakedObjectsLoading,
        unlockAllsupplyIncreaseVesting,
    } = useGetSupplyIncreaseVestingObjects(address);

    const timelockedStakedObjectsGrouped: TimelockedStakedObjectsGrouped[] =
        groupTimelockedStakedObjects(supplyIncreaseVestingStakedMapped || []);

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

    const formattedLastPayoutExpirationTime = useCountdownByTimestamp(
        Number(nextPayout?.expirationTimestampMs),
    );

    const [formattedTotalVested, vestedSymbol] = useFormatCoin(
        supplyIncreaseVestingSchedule.totalVested,
        IOTA_TYPE_ARG,
    );

    const [formattedTotalLocked, lockedSymbol] = useFormatCoin(
        supplyIncreaseVestingSchedule.totalLocked,
        IOTA_TYPE_ARG,
    );

    const [formattedAvailableClaiming, availableClaimingSymbol] = useFormatCoin(
        supplyIncreaseVestingSchedule.availableClaiming,
        IOTA_TYPE_ARG,
    );

    const [totalStakedFormatted, totalStakedSymbol] = useFormatCoin(
        supplyIncreaseVestingSchedule.totalStaked,
        IOTA_TYPE_ARG,
    );

    const [totalEarnedFormatted, totalEarnedSymbol] = useFormatCoin(
        supplyIncreaseVestingSchedule.totalEarned,
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
        if (!unlockAllsupplyIncreaseVesting?.transactionBlock) {
            addNotification('Failed to create a Transaction', NotificationType.Error);
            return;
        }
        signAndExecuteTransaction(
            {
                transaction: unlockAllsupplyIncreaseVesting.transactionBlock,
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

    function openReceiveTokenPopup(): void {
        setIsVestingScheduleDialogOpen(true);
    }

    useEffect(() => {
        if (!supplyIncreaseVestingEnabled) {
            router.push('/');
        }
    }, [router, supplyIncreaseVestingEnabled]);

    if (istimelockedStakedObjectsLoading) {
        return (
            <div className="flex w-full max-w-4xl items-start justify-center justify-self-center">
                <LoadingIndicator />
            </div>
        );
    }

    return (
        <div className="flex w-full max-w-4xl flex-col items-stretch justify-center gap-lg justify-self-center md:flex-row">
            <div className="flex w-full flex-col gap-lg md:w-1/2">
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
                                    !supplyIncreaseVestingSchedule.availableClaiming ||
                                    supplyIncreaseVestingSchedule.availableClaiming === 0n
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
                                onClick={openReceiveTokenPopup}
                                title="See All"
                                buttonType={ButtonType.Secondary}
                                buttonDisabled={!supplyIncreaseVestingPortfolio}
                            />
                        </Card>
                        {supplyIncreaseVestingPortfolio && (
                            <VestingScheduleDialog
                                open={isVestingScheduleDialogOpen}
                                setOpen={setIsVestingScheduleDialogOpen}
                                vestingPortfolio={supplyIncreaseVestingPortfolio}
                            />
                        )}
                    </div>
                </Panel>

                {supplyIncreaseVestingMapped.length === 0 ? (
                    <Banner
                        videoSrc={videoSrc}
                        title="Stake Vested Tokens"
                        subtitle="Earn Rewards"
                        onButtonClick={() => handleNewStake()}
                        buttonText="Stake"
                    />
                ) : null}
            </div>

            {supplyIncreaseVestingStakedMapped.length !== 0 ? (
                <div className="flex w-full md:w-1/2">
                    <Panel>
                        <Title
                            title="Staked Vesting"
                            trailingElement={
                                <Button
                                    type={ButtonType.Primary}
                                    text="Stake"
                                    onClick={() => {
                                        setStakeDialogView(StakeDialogView.SelectValidator);
                                    }}
                                />
                            }
                        />

                        <div className="flex flex-col px-lg py-sm">
                            <div className="flex flex-row gap-md">
                                <DisplayStats
                                    label="Your stake"
                                    value={`${totalStakedFormatted} ${totalStakedSymbol}`}
                                />
                                <DisplayStats
                                    label="Earned"
                                    value={`${totalEarnedFormatted} ${totalEarnedSymbol}`}
                                />
                            </div>
                        </div>
                        <div className="flex flex-col px-lg py-sm">
                            <div className="flex w-full flex-col items-center justify-center space-y-4 pt-4">
                                {system &&
                                    timelockedStakedObjectsGrouped?.map(
                                        (timelockedStakedObject) => {
                                            return (
                                                <StakedTimelockObject
                                                    key={
                                                        timelockedStakedObject.validatorAddress +
                                                        timelockedStakedObject.stakeRequestEpoch +
                                                        timelockedStakedObject.label
                                                    }
                                                    getValidatorByAddress={getValidatorByAddress}
                                                    timelockedStakedObject={timelockedStakedObject}
                                                    handleUnstake={handleUnstake}
                                                    currentEpoch={Number(system.epoch)}
                                                />
                                            );
                                        },
                                    )}
                            </div>
                        </div>
                    </Panel>
                </div>
            ) : null}
            <StakeDialog
                isTimelockedStaking
                stakedDetails={selectedStake}
                onSuccess={handleOnSuccess}
                isOpen={isDialogStakeOpen}
                handleClose={handleCloseStakeDialog}
                view={stakeDialogView}
                setView={setStakeDialogView}
                selectedValidator={selectedValidator}
                setSelectedValidator={setSelectedValidator}
                maxStakableTimelockedAmount={BigInt(supplyIncreaseVestingSchedule.availableStaking)}
            />
        </div>
    );
}

export default VestingDashboardPage;
