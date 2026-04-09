// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import {
    StakeDialog,
    useStakeDialog,
    VestingScheduleDialog,
    UnstakeDialog,
    SupplyIncreaseVestingOverview,
    StakeDialogView,
    CollectTransactionDialog,
} from '@/components';
import { UnstakeDialogView } from '@/components/dialogs/unstake/enums';
import { useUnstakeDialog } from '@/components/dialogs/unstake/hooks';
import { useGetSupplyIncreaseVestingObjects } from '@/hooks';
import { groupTimelockedStakedObjects, TimelockedStakedObjectsGrouped } from '@/lib/utils';
import { SupplyIncreaseUserType } from '@/lib/interfaces';
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
    ButtonSize,
} from '@iota/apps-ui-kit';
import {
    Theme,
    useFormatCoin,
    useTheme,
    useCountdownByTimestamp,
    toast,
    useBalance,
    GAS_BUDGET_ERROR_MESSAGES,
    GAS_BALANCE_TOO_LOW_ID,
    MIN_NUMBER_IOTA_TO_STAKE,
    NOT_ENOUGH_BALANCE_ID,
    Banner,
} from '@iota/core';
import {
    useCurrentAccount,
    useIotaClient,
    useIotaClientQuery,
    useSignAndExecuteTransaction,
} from '@iota/dapp-kit';
import { IotaValidatorSummary } from '@iota/iota-sdk/client';
import { Calendar, StarHex, Warning } from '@iota/apps-ui-icons';
import { useEffect, useState } from 'react';
import { StakedTimelockObject } from '@/components';
import { IotaSignAndExecuteTransactionOutput } from '@iota/wallet-standard';
import { ampli } from '@/lib/utils/analytics';
import BigNumber from 'bignumber.js';

export default function VestingDashboardPage(): JSX.Element {
    const [timelockedObjectsToUnstake, setTimelockedObjectsToUnstake] =
        useState<TimelockedStakedObjectsGrouped | null>(null);
    const [collectTxDigest, setCollectTxDigest] = useState<string | null>(null);
    const [showCollectTransaction, setShowCollectTransaction] = useState(false);
    const account = useCurrentAccount();
    const address = account?.address || '';
    const iotaClient = useIotaClient();
    const { data: system } = useIotaClientQuery('getLatestIotaSystemState');
    const [isVestingScheduleDialogOpen, setIsVestingScheduleDialogOpen] = useState(false);
    const { mutateAsync: signAndExecuteTransaction, isPending: isSendingTransaction } =
        useSignAndExecuteTransaction();
    const { theme } = useTheme();
    const { data: balance } = useBalance(address);

    const videoSrc =
        theme === Theme.Dark
            ? 'https://files.iota.org/media/tooling/wallet-dashboard-staking-dark.mp4'
            : 'https://files.iota.org/media/tooling/wallet-dashboard-staking-light.mp4';

    const {
        nextPayout,
        supplyIncreaseVestingPortfolio,
        supplyIncreaseVestingSchedule,
        supplyIncreaseVestingMapped,
        supplyIncreaseVestingStakedMapped,
        isTimelockedStakedObjectsLoading,
        unlockAllSupplyIncreaseVesting,
        refreshStakeList,
        isSupplyIncreaseVestingScheduleEmpty,
        isMaxTransactionSizeError,
        supplyIncreaseVestingUnlockedMaxSize,
        isUnlockPending,
        resetMaxTransactionSize,
        isUnlockError,
        unlockError,
        userType,
        inactiveValidatorUnlockedStakes,
    } = useGetSupplyIncreaseVestingObjects(address);

    const timelockedStakedObjectsGrouped: TimelockedStakedObjectsGrouped[] =
        groupTimelockedStakedObjects(supplyIncreaseVestingStakedMapped || []);

    const inactiveValidatorAddresses = new Set(
        inactiveValidatorUnlockedStakes.map((stake) => stake.validatorAddress),
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

    const {
        isOpen: isUnstakeDialogOpen,
        openUnstakeDialog,
        defaultDialogProps,
        setTxDigest,
        setView: setUnstakeDialogView,
    } = useUnstakeDialog();

    useEffect(() => {
        if (isUnlockError && unlockError) {
            console.error('[DEBUG]: Vesting unlock Error:', unlockError);
        }
    }, [unlockError, isUnlockError]);

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
        return system?.activeValidators?.find(
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
        if (isUnlockError && unlockError?.message.includes(NOT_ENOUGH_BALANCE_ID)) {
            toast.error(GAS_BUDGET_ERROR_MESSAGES[NOT_ENOUGH_BALANCE_ID]);
            return;
        }

        if (
            new BigNumber(balance?.totalBalance || 0).lt(
                unlockAllSupplyIncreaseVesting?.transactionBlock?.getData?.().gasData?.budget || 0,
            )
        ) {
            toast.error(GAS_BUDGET_ERROR_MESSAGES[GAS_BALANCE_TOO_LOW_ID]);
            return;
        }

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
                    setCollectTxDigest(tx.digest);
                    setShowCollectTransaction(true);
                    ampli.timelockCollect();
                    toast.success('Collect transaction has been sent');

                    if (isMaxTransactionSizeError) {
                        resetMaxTransactionSize();
                    }
                },
            },
        ).catch((error) => {
            toast.error('Collect transaction was not sent');
            console.error('Error executing collect transaction:', error);
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

    // Simplified UI for Staker users
    if (userType === SupplyIncreaseUserType.Staker) {
        return (
            <>
                <div className="flex w-full flex-col items-center justify-center gap-lg justify-self-center">
                    <div className="flex w-full flex-col gap-lg md:w-3/4">
                        <SupplyIncreaseVestingOverview
                            customButton={
                                <Button
                                    type={ButtonType.Primary}
                                    onClick={handleCollect}
                                    text="Collect"
                                    icon={
                                        hasAvailableClaiming && isUnlockPending ? (
                                            <LoadingIndicator />
                                        ) : undefined
                                    }
                                    disabled={
                                        !supplyIncreaseVestingSchedule.availableClaiming ||
                                        supplyIncreaseVestingSchedule.availableClaiming === 0n ||
                                        isUnlockPending ||
                                        inactiveValidatorUnlockedStakes.length > 0 ||
                                        isSendingTransaction
                                    }
                                    fullWidth
                                />
                            }
                        />
                        <Panel>
                            <Title
                                title="Vesting"
                                size={TitleSize.Medium}
                                trailingElement={
                                    <div className="flex flex-row gap-xs">
                                        <Button
                                            type={ButtonType.Secondary}
                                            onClick={openReceiveTokenDialog}
                                            text="Rewards Schedule"
                                            icon={<StarHex />}
                                            disabled={!supplyIncreaseVestingPortfolio}
                                            size={ButtonSize.Small}
                                        />
                                    </div>
                                }
                            />
                            <div className="flex flex-col gap-md p-lg pt-sm">
                                <div className="flex h-24 flex-row gap-md">
                                    <DisplayStats
                                        label="Total Vested"
                                        value={formattedTotalVested}
                                        supportingLabel={vestedSymbol}
                                    />
                                    <DisplayStats
                                        label="Available Rewards"
                                        value={formattedAvailableClaiming}
                                        supportingLabel={availableClaimingSymbol}
                                        tooltipText="Total amount of IOTA that is available to collect."
                                        tooltipPosition={TooltipPosition.Right}
                                    />
                                </div>
                                {isMaxTransactionSizeError ? (
                                    <InfoBox
                                        title="Partial collect"
                                        supportingText={`Due to the large number of objects, a partial collect will be attempted for ${formattedSupplyIncreaseVestingUnlockedMaxSize} ${supplyIncreaseVestingUnlockedMaxSizeSymbol}. After the operation is complete, you can collect the remaining value.`}
                                        style={InfoBoxStyle.Elevated}
                                        type={InfoBoxType.Warning}
                                        icon={<Warning />}
                                    />
                                ) : null}
                                {supplyIncreaseVestingPortfolio && (
                                    <VestingScheduleDialog
                                        open={isVestingScheduleDialogOpen}
                                        setOpen={setIsVestingScheduleDialogOpen}
                                        vestingPortfolio={supplyIncreaseVestingPortfolio}
                                        userType={userType}
                                    />
                                )}
                            </div>
                        </Panel>
                    </div>

                    {!isSupplyIncreaseVestingScheduleEmpty &&
                    supplyIncreaseVestingSchedule.totalStaked !== 0n ? (
                        <div className="flex w-full md:w-3/4">
                            <Panel>
                                <Title title="Staked Vesting" />

                                <div className="flex flex-col gap-y-md px-lg py-sm">
                                    {inactiveValidatorUnlockedStakes.length > 0 && (
                                        <InfoBox
                                            title="Inactive validator"
                                            supportingText="Some timelocked stakes cannot be collected because their validator is no longer active. Please unstake them first."
                                            style={InfoBoxStyle.Elevated}
                                            type={InfoBoxType.Warning}
                                            icon={<Warning />}
                                        />
                                    )}
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
                                                            showUnstakeButton={inactiveValidatorAddresses.has(
                                                                timelockedStakedObject.validatorAddress,
                                                            )}
                                                        />
                                                    );
                                                },
                                            )}
                                    </div>
                                </div>
                            </Panel>
                        </div>
                    ) : null}
                </div>
                <UnstakeDialog
                    {...defaultDialogProps}
                    groupedTimelockedObjects={timelockedObjectsToUnstake || undefined}
                    onSuccess={handleOnSuccessUnstake}
                />
                <StakeDialog
                    isTimelockedStaking={true}
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
                {showCollectTransaction && collectTxDigest && (
                    <CollectTransactionDialog
                        open={showCollectTransaction}
                        txDigest={collectTxDigest}
                        onClose={() => {
                            setShowCollectTransaction(false);
                            refreshStakeList();
                        }}
                    />
                )}
            </>
        );
    }

    // Full UI for Entity users (investors) - original structure
    return (
        <>
            <div className="flex w-full flex-col items-center justify-center gap-lg justify-self-center">
                <div className="flex w-full flex-col gap-lg md:w-3/4">
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
                                    <StarHex className="h-5 w-5 text-iota-primary-30 dark:text-iota-primary-80" />
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
                                    type={InfoBoxType.Warning}
                                    icon={<Warning />}
                                />
                            ) : null}
                            <Card type={CardType.Outlined}>
                                <CardImage
                                    type={ImageType.BgSolid}
                                    shape={ImageShape.SquareRounded}
                                >
                                    <Calendar className="h-5 w-5 text-iota-primary-30 dark:text-iota-primary-80" />
                                </CardImage>
                                <CardBody
                                    title={`${formattedNextPayout} ${nextPayoutSymbol}`}
                                    subtitle={`Next payout ${nextPayout?.expirationTimestampMs ? formattedLastPayoutExpirationTime : ''}`}
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
                                    userType={userType}
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
                        >
                            <Button
                                onClick={() => handleNewStake()}
                                size={ButtonSize.Small}
                                type={ButtonType.Outlined}
                                text="Stake"
                                disabled={supplyIncreaseVestingSchedule.availableStaking === 0n}
                            />
                        </Banner>
                    ) : null}
                </div>

                {!isSupplyIncreaseVestingScheduleEmpty &&
                supplyIncreaseVestingSchedule.totalStaked !== 0n ? (
                    <div className="flex w-full md:w-3/4">
                        <Panel>
                            <Title
                                title="Staked Vesting"
                                trailingElement={
                                    <Button
                                        type={ButtonType.Primary}
                                        text="Stake"
                                        onClick={() => {
                                            if (
                                                supplyIncreaseVestingSchedule.availableStaking ===
                                                    0n ||
                                                new BigNumber(formattedAvailableStaking).lt(
                                                    MIN_NUMBER_IOTA_TO_STAKE,
                                                )
                                            ) {
                                                toast.error(
                                                    'Not enough funds available for staking',
                                                );
                                                return;
                                            }
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
