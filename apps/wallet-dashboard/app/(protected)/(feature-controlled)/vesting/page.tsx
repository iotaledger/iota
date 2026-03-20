// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import {
    StakeDialog,
    useStakeDialog,
    VestingScheduleDialog,
    UnstakeDialog,
    SupplyIncreaseVestingOverview,
} from '@/components';
import { UnstakeDialogView } from '@/components/dialogs/unstake/enums';
import { useUnstakeDialog } from '@/components/dialogs/unstake/hooks';
import { useGetSupplyIncreaseVestingObjects } from '@/hooks';
import { groupTimelockedStakedObjects, TimelockedStakedObjectsGrouped } from '@/lib/utils';
import {
    Panel,
    Title,
    TitleSize,
    DisplayStats,
    TooltipPosition,
    Card,
    CardBody,
    CardType,
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
    useFormatCoin,
    toast,
    useBalance,
    GAS_BUDGET_ERROR_MESSAGES,
    GAS_BALANCE_TOO_LOW_ID,
    NOT_ENOUGH_BALANCE_ID,
} from '@iota/core';
import {
    useCurrentAccount,
    useIotaClient,
    useIotaClientQuery,
    useSignAndExecuteTransaction,
} from '@iota/dapp-kit';
import { IotaValidatorSummary } from '@iota/iota-sdk/client';
import { StarHex, Warning } from '@iota/apps-ui-icons';
import { useEffect, useState } from 'react';
import { StakedTimelockObject } from '@/components';
import { IotaSignAndExecuteTransactionOutput } from '@iota/wallet-standard';
import { ampli } from '@/lib/utils/analytics';
import clsx from 'clsx';
import BigNumber from 'bignumber.js';

export default function VestingDashboardPage(): JSX.Element {
    const [timelockedObjectsToUnstake, setTimelockedObjectsToUnstake] =
        useState<TimelockedStakedObjectsGrouped | null>(null);
    const account = useCurrentAccount();
    const address = account?.address || '';
    const iotaClient = useIotaClient();
    const { data: system } = useIotaClientQuery('getLatestIotaSystemState');
    const [isVestingScheduleDialogOpen, setIsVestingScheduleDialogOpen] = useState(false);
    const { mutateAsync: signAndExecuteTransaction } = useSignAndExecuteTransaction();
    const { data: balance } = useBalance(address);

    const {
        supplyIncreaseVestingPortfolio,
        supplyIncreaseVestingSchedule,
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

    const [formattedTotalVested, vestedSymbol] = useFormatCoin({
        balance: supplyIncreaseVestingSchedule.totalVested,
    });

    const [formattedAvailableClaiming, availableClaimingSymbol] = useFormatCoin({
        balance: supplyIncreaseVestingSchedule.availableClaiming,
    });

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
            .catch((error) => {
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
                                    isUnlockPending
                                }
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
                                />
                            )}
                        </div>
                    </Panel>
                </div>

                {!isSupplyIncreaseVestingScheduleEmpty &&
                supplyIncreaseVestingSchedule.totalStaked !== 0n ? (
                    <div className="flex w-full md:w-1/2">
                        <Panel>
                            <Title title="Staked Vesting" />

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
