// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import {
    Banner,
    StakeDialog,
    useStakeDialog,
    VestingScheduleDialog,
    UnstakeDialog,
    StakeDialogView,
} from '@/components';
import { UnstakeDialogView } from '@/components/dialogs/unstake/enums';
import { useUnstakeDialog } from '@/components/dialogs/unstake/hooks';
import { useGetSupplyIncreaseVestingObjects } from '@/hooks';
import { groupTimelockedStakedObjects, TimelockedStakedObjectsGrouped } from '@/lib/utils';
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
    LabelText,
    LabelTextSize,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
} from '@iota/apps-ui-kit';
import {
    Theme,
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
import { Calendar, StarHex, Warning } from '@iota/apps-ui-icons';
import { useRouter } from 'next/navigation';
import { useEffect, useState } from 'react';
import { StakedTimelockObject } from '@/components';
import { IotaSignAndExecuteTransactionOutput } from '@iota/wallet-standard';
import toast from 'react-hot-toast';
import { ampli } from '@/lib/utils/analytics';
import clsx from 'clsx';

export default function VestingDashboardPage(): JSX.Element {
    const [timelockedObjectsToUnstake, setTimelockedObjectsToUnstake] =
        useState<TimelockedStakedObjectsGrouped | null>(null);
    const account = useCurrentAccount();
    const address = account?.address || '';
    const iotaClient = useIotaClient();
    const router = useRouter();
    const { data: system } = useIotaClientQuery('getLatestIotaSystemState');
    const [isVestingScheduleDialogOpen, setIsVestingScheduleDialogOpen] = useState(false);
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
        supplyIncreaseVestingStakedMapped,
        isTimelockedStakedObjectsLoading,
        unlockAllSupplyIncreaseVesting,
        refreshStakeList,
        isSupplyIncreaseVestingScheduleEmpty,
        supplyIncreaseVestingMapped,
        isMaxTransactionSizeError,
        supplyIncreaseVestingUnlockedMaxSize,
        isUnlockPending,
        resetMaxTransactionSize,
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

    const {
        isOpen: isUnstakeDialogOpen,
        openUnstakeDialog,
        defaultDialogProps,
        setTxDigest,
        setView: setUnstakeDialogView,
    } = useUnstakeDialog();

    const formattedLastPayoutExpirationTime = useCountdownByTimestamp(
        Number(nextPayout?.expirationTimestampMs),
        {
            hideZeroUnits: true,
        },
    );

    const [formattedTotalVested, vestedSymbol] = useFormatCoin({
        balance: supplyIncreaseVestingSchedule.totalVested,
    });

    const [formattedTotalLocked, lockedSymbol] = useFormatCoin({
        balance: supplyIncreaseVestingSchedule.totalLocked,
    });

    const [formattedAvailableClaiming, availableClaimingSymbol] = useFormatCoin({
        balance: supplyIncreaseVestingSchedule.availableClaiming,
    });

    const [formattedNextPayout, nextPayoutSymbol] = useFormatCoin({ balance: nextPayout?.amount });

    function getValidatorByAddress(validatorAddress: string): IotaValidatorSummary | undefined {
        return activeValidators?.find(
            (activeValidator) => activeValidator.iotaAddress === validatorAddress,
        );
    }

    const [totalStakedFormatted, totalStakedSymbol] = useFormatCoin({
        balance: supplyIncreaseVestingSchedule.totalStaked,
    });

    const [totalEarnedFormatted, totalEarnedSymbol] = useFormatCoin({
        balance: supplyIncreaseVestingSchedule.totalEarned,
    });

    const [formattedAvailableStaking, availableStakingSymbol] = useFormatCoin({
        balance: supplyIncreaseVestingSchedule.availableStaking,
    });

    const [
        formattedSupplyIncreaseVestingUnlockedMaxSize,
        supplyIncreaseVestingUnlockedMaxSizeSymbol,
    ] = useFormatCoin({ balance: supplyIncreaseVestingUnlockedMaxSize });

    function handleOnSuccess(digest: string): void {
        setTimelockedObjectsToUnstake(null);

        iotaClient
            .waitForTransaction({
                digest,
            })
            .then(refreshStakeList);
    }

    const handleCollect = () => {
        if (!unlockAllSupplyIncreaseVesting?.transactionBlock) {
            toast.error('Failed to create a Transaction');
            return;
        }
        signAndExecuteTransaction(
            {
                transaction: unlockAllSupplyIncreaseVesting.transactionBlock,
            },
            {
                onSuccess: (tx) => {
                    handleOnSuccess(tx.digest);
                    ampli.timelockCollect();

                    if (isMaxTransactionSizeError) {
                        resetMaxTransactionSize();
                    }
                },
            },
        )
            .then(() => {
                toast.success('Collect transaction has been sent');
            })
            .catch(() => {
                toast.error('Collect transaction was not sent');
            });
    };

    function handleUnstake(delegatedTimelockedStake: TimelockedStakedObjectsGrouped): void {
        setTimelockedObjectsToUnstake(delegatedTimelockedStake);
        openUnstakeDialog(UnstakeDialogView.TimelockedUnstake);
    }

    function openReceiveTokenDialog(): void {
        setIsVestingScheduleDialogOpen(true);
    }

    function handleOnSuccessUnstake(tx: IotaSignAndExecuteTransactionOutput): void {
        setUnstakeDialogView(UnstakeDialogView.TransactionDetails);
        iotaClient.waitForTransaction({ digest: tx.digest }).then((tx) => {
            refreshStakeList();
            setTxDigest(tx.digest);
        });
    }

    useEffect(() => {
        if (!supplyIncreaseVestingEnabled) {
            router.push('/home');
        }
    }, [router, supplyIncreaseVestingEnabled]);

    if (isTimelockedStakedObjectsLoading) {
        return (
            <div className="flex w-full max-w-4xl items-start justify-center justify-self-center">
                <LoadingIndicator />
            </div>
        );
    }

    const hasAvailableClaiming =
        !!supplyIncreaseVestingSchedule.availableClaiming &&
        supplyIncreaseVestingSchedule.availableClaiming !== 0n;

    return (
        <>
            <div className="flex w-full flex-col items-stretch justify-center gap-lg justify-self-center md:flex-row">
                <div
                    className={clsx(
                        'flex w-full flex-col gap-lg',
                        !isSupplyIncreaseVestingScheduleEmpty &&
                            supplyIncreaseVestingSchedule.totalStaked !== 0n
                            ? 'md:w-1/2'
                            : 'md:w-2/3',
                    )}
                >
                    <Panel>
                        <Title title="Vesting" size={TitleSize.Medium} />
                        <div className="flex flex-col gap-md p-lg pt-sm">
                            <div className="flex h-24 flex-row gap-md">
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
                                <CardImage
                                    type={ImageType.BgSolid}
                                    shape={ImageShape.SquareRounded}
                                >
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
                                    icon={
                                        hasAvailableClaiming && isUnlockPending ? (
                                            <LoadingIndicator />
                                        ) : null
                                    }
                                    buttonDisabled={
                                        !supplyIncreaseVestingSchedule.availableClaiming ||
                                        supplyIncreaseVestingSchedule.availableClaiming === 0n ||
                                        isUnlockPending
                                    }
                                />
                            </Card>
                            {isMaxTransactionSizeError ? (
                                <InfoBox
                                    title="Partial collect"
                                    supportingText={`Due to the large number of objects, a partial collect will be attempted for ${formattedSupplyIncreaseVestingUnlockedMaxSize} ${supplyIncreaseVestingUnlockedMaxSizeSymbol}. After the operation is complete, you can collect the remaining value.`}
                                    style={InfoBoxStyle.Elevated}
                                    type={InfoBoxType.Error}
                                    icon={<Warning />}
                                />
                            ) : null}
                            <Card type={CardType.Outlined}>
                                <CardImage
                                    type={ImageType.BgSolid}
                                    shape={ImageShape.SquareRounded}
                                >
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
                                    onClick={openReceiveTokenDialog}
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

                    {supplyIncreaseVestingMapped.length > 0 &&
                    supplyIncreaseVestingSchedule.totalStaked === 0n ? (
                        <Banner
                            videoSrc={videoSrc}
                            title="Stake Vested Tokens"
                            subtitle="Earn Rewards"
                            onButtonClick={() => handleNewStake()}
                            buttonText="Stake"
                            disabled={supplyIncreaseVestingSchedule.availableStaking === 0n}
                        />
                    ) : null}
                </div>

                {!isSupplyIncreaseVestingScheduleEmpty &&
                supplyIncreaseVestingSchedule.totalStaked !== 0n ? (
                    <div className="flex w-full md:w-1/2">
                        <Panel>
                            <Title
                                title="Staked Vesting"
                                trailingElement={
                                    <Button
                                        type={ButtonType.Primary}
                                        text="Stake"
                                        disabled={
                                            supplyIncreaseVestingSchedule.availableStaking === 0n
                                        }
                                        onClick={() => {
                                            setStakeDialogView(StakeDialogView.SelectValidator);
                                        }}
                                    />
                                }
                            />

                            <div className="flex flex-col gap-y-md px-lg py-sm">
                                <div className="flex flex-row gap-x-md">
                                    <DisplayStats
                                        label="Your stake"
                                        value={`${totalStakedFormatted} ${totalStakedSymbol}`}
                                    />
                                    <DisplayStats
                                        label="Earned"
                                        value={`${totalEarnedFormatted} ${totalEarnedSymbol}`}
                                    />
                                </div>
                                <div className="flex w-full">
                                    <Card type={CardType.Filled}>
                                        <CardBody
                                            title=""
                                            subtitle={
                                                <LabelText
                                                    size={LabelTextSize.Large}
                                                    label="Available for staking"
                                                    text={formattedAvailableStaking}
                                                    supportingLabel={availableStakingSymbol}
                                                />
                                            }
                                        />
                                    </Card>
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
                                                        getValidatorByAddress={
                                                            getValidatorByAddress
                                                        }
                                                        timelockedStakedObject={
                                                            timelockedStakedObject
                                                        }
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

                {isDialogStakeOpen && (
                    <StakeDialog
                        isTimelockedStaking
                        stakedDetails={selectedStake}
                        onSuccess={handleOnSuccess}
                        handleClose={handleCloseStakeDialog}
                        view={stakeDialogView}
                        setView={setStakeDialogView}
                        selectedValidator={selectedValidator}
                        setSelectedValidator={setSelectedValidator}
                        maxStakableTimelockedAmount={BigInt(
                            supplyIncreaseVestingSchedule.availableStaking,
                        )}
                    />
                )}

                {isUnstakeDialogOpen && timelockedObjectsToUnstake && (
                    <UnstakeDialog
                        groupedTimelockedObjects={timelockedObjectsToUnstake}
                        onSuccess={handleOnSuccessUnstake}
                        {...defaultDialogProps}
                    />
                )}
            </div>
        </>
    );
}
