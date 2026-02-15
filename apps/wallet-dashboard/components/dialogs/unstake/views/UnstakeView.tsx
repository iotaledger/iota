// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Header,
    Button,
    KeyValueInfo,
    ButtonType,
    Panel,
    LoadingIndicator,
    InfoBoxType,
    InfoBoxStyle,
    InfoBox,
    Input,
    InputType,
    Divider,
} from '@iota/apps-ui-kit';
import {
    ExtendedDelegatedStake,
    GAS_SYMBOL,
    useFormatCoin,
    useGetStakingValidatorDetails,
    useNewUnstakeTransaction,
    useNewPartialUnstakeTransaction,
    Validator,
    toast,
    NOT_ENOUGH_BALANCE_ID,
    GAS_BUDGET_ERROR_MESSAGES,
    GAS_BALANCE_TOO_LOW_ID,
} from '@iota/core';
import { CoinFormat, NANOS_PER_IOTA } from '@iota/iota-sdk/utils';
import { useCurrentAccount, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { Warning, Info } from '@iota/apps-ui-icons';
import { ValidatorStakingData } from '@/components';
import { DialogLayout, DialogLayoutFooter, DialogLayoutBody } from '../../layout';

import { IotaSignAndExecuteTransactionOutput } from '@iota/wallet-standard';
import { ampli } from '@/lib/utils/analytics';
import { useEffect, useState } from 'react';

interface UnstakeDialogProps {
    extendedStake: ExtendedDelegatedStake;
    handleClose: () => void;
    onSuccess: (tx: IotaSignAndExecuteTransactionOutput) => void;
    showActiveStatus?: boolean;
    onBack?: () => void;
}

export function UnstakeView({
    extendedStake,
    handleClose,
    onBack,
    onSuccess,
    showActiveStatus,
}: UnstakeDialogProps): JSX.Element {
    const activeAddress = useCurrentAccount()?.address ?? '';
    const [partialUnstakeAmount, setPartialUnstakeAmount] = useState<string>('');
    const [isPartialUnstake, setIsPartialUnstake] = useState(false);

    // Parse the unstake amount in nanos
    const parsedAmount = parseFloat(partialUnstakeAmount);
    const unstakeAmountNanos =
        partialUnstakeAmount && !isNaN(parsedAmount) && parsedAmount > 0
            ? BigInt(Math.floor(parsedAmount * Number(NANOS_PER_IOTA)))
            : 0n;

    const {
        data: unstakeData,
        isPending: isUnstakeTxPending,
        error,
        isError: isUnstakeError,
    } = useNewUnstakeTransaction(activeAddress, extendedStake.stakedIotaId);

    const {
        data: partialUnstakeData,
        isPending: isPartialUnstakeTxPending,
        error: partialError,
        isError: isPartialUnstakeError,
    } = useNewPartialUnstakeTransaction(
        activeAddress,
        extendedStake.stakedIotaId,
        unstakeAmountNanos,
    );

    // Use partial unstake data if enabled, otherwise use full unstake
    const activeUnstakeData =
        isPartialUnstake && unstakeAmountNanos > 0n ? partialUnstakeData : unstakeData;
    const activeError = isPartialUnstake ? partialError : error;
    const activeIsError = isPartialUnstake ? isPartialUnstakeError : isUnstakeError;
    const activeIsPending = isPartialUnstake ? isPartialUnstakeTxPending : isUnstakeTxPending;

    const [gasFormatted] = useFormatCoin({
        balance: activeUnstakeData?.gasSummary?.totalGas,
        format: CoinFormat.Full,
    });

    const { mutateAsync: signAndExecuteTransaction, isPending: isTransactionPending } =
        useSignAndExecuteTransaction();

    const { systemDataResult, delegatedStakeDataResult } = useGetStakingValidatorDetails({
        accountAddress: activeAddress,
        validatorAddress: extendedStake.validatorAddress,
        stakeId: extendedStake.stakedIotaId,
        unstake: true,
    });

    // Calculate the amount to unstake and proportional rewards
    const principalAmount = BigInt(extendedStake.principal);
    const rewardAmount = BigInt(extendedStake.estimatedReward || 0);

    const unstakeAmount =
        isPartialUnstake && unstakeAmountNanos > 0n ? unstakeAmountNanos : principalAmount;

    // Calculate proportional rewards for partial unstake
    const proportionalRewards =
        principalAmount > 0n ? (rewardAmount * unstakeAmount) / principalAmount : 0n;

    const totalUnstakeAmount = unstakeAmount + proportionalRewards;

    const remainingStake = principalAmount - unstakeAmount;
    const remainingRewards = rewardAmount - proportionalRewards;
    const remainingTotalStaked = remainingStake + remainingRewards;
    const [remainingStakeFormatted] = useFormatCoin({ balance: remainingStake });
    const [remainingRewardsFormatted, remainingRewardsSymbol] = useFormatCoin({
        balance: remainingRewards,
    });
    const [remainingTotalStakedFormatted] = useFormatCoin({ balance: remainingTotalStaked });
    const [unstakeAmountFormatted] = useFormatCoin({ balance: unstakeAmount });
    const [rewardsFormatted, rewardSymbol] = useFormatCoin({ balance: proportionalRewards });
    const [totalUnstakeAmountFormatted] = useFormatCoin({ balance: totalUnstakeAmount });

    useEffect(() => {
        if ((isUnstakeError && error) || (isPartialUnstakeError && partialError)) {
            console.error('[DEBUG]: Unstake Error:', activeError);
        }
    }, [isUnstakeError, error, isPartialUnstakeError, partialError, activeError]);

    const { isLoading: loadingValidators, error: errorValidators } = systemDataResult;
    const {
        isLoading: isLoadingDelegatedStakeData,
        isError,
        error: delegatedStakeDataError,
    } = delegatedStakeDataResult;

    const delegationId = extendedStake?.stakedIotaId;
    const isNotEnoughGas =
        activeError &&
        (activeError.message.includes(NOT_ENOUGH_BALANCE_ID) ||
            activeError.message.includes(GAS_BALANCE_TOO_LOW_ID));

    const maxUnstakeAmount = Number(principalAmount) / Number(NANOS_PER_IOTA);
    const MIN_STAKING_THRESHOLD = 1_000_000_000n; // 1 IOTA in nanos

    // For partial unstake:
    // - Unstake amount must be >= MIN_STAKING_THRESHOLD
    // - Remaining amount must be >= MIN_STAKING_THRESHOLD
    // For full unstake:
    // - Remaining amount must be >= MIN_STAKING_THRESHOLD (i.e., cannot unstake everything)
    const remainingAmount = principalAmount - unstakeAmountNanos;
    const isInvalidAmount =
        (isPartialUnstake &&
            (unstakeAmountNanos <= 0n ||
                unstakeAmountNanos > principalAmount ||
                unstakeAmountNanos < MIN_STAKING_THRESHOLD ||
                remainingAmount < MIN_STAKING_THRESHOLD)) ||
        (!isPartialUnstake && remainingAmount < MIN_STAKING_THRESHOLD);

    // Determine the appropriate error message
    const getErrorMessage = () => {
        if (!isInvalidAmount) return undefined;

        if (!isPartialUnstake) {
            return 'You must leave at least 1 IOTA staked';
        }

        if (unstakeAmountNanos <= 0n) {
            return 'Amount must be greater than 0';
        }
        if (unstakeAmountNanos > principalAmount) {
            return `Amount cannot exceed ${maxUnstakeAmount.toFixed(2)} IOTA`;
        }
        if (unstakeAmountNanos < MIN_STAKING_THRESHOLD) {
            return 'Unstake amount must be at least 1 IOTA';
        }
        if (remainingAmount < MIN_STAKING_THRESHOLD) {
            return 'Remaining stake must be at least 1 IOTA';
        }
        return undefined;
    };

    const validatorName =
        systemDataResult.data?.activeValidators.find(
            (v) => v.iotaAddress === extendedStake.validatorAddress,
        )?.name ?? '';

    const [stakedFormattedPlain] = useFormatCoin({
        balance: unstakeAmount,
        format: CoinFormat.Full,
        useGroupSeparator: false,
    });

    const [rewardsFormattedPlain] = useFormatCoin({
        balance: extendedStake.estimatedReward,
        format: CoinFormat.Full,
        useGroupSeparator: false,
    });

    async function handleUnstake(): Promise<void> {
        if (!activeUnstakeData) return;

        await signAndExecuteTransaction(
            {
                transaction: activeUnstakeData.transaction,
            },
            {
                onSuccess: (tx) => {
                    toast.success('Unstake transaction has been sent');
                    onSuccess(tx);

                    ampli.iotaUnstaked({
                        stakedAmount: Number(stakedFormattedPlain),
                        validatorAddress: extendedStake.validatorAddress,
                        rewards: Number(rewardsFormattedPlain),
                        validatorName,
                    });
                },
            },
        ).catch((error) => {
            toast.error('Unstake transaction was not sent');
            console.error('Error executing unstake transaction:', error);
        });
    }

    if (isLoadingDelegatedStakeData || loadingValidators) {
        return (
            <div className="flex h-full w-full items-center justify-center p-2">
                <LoadingIndicator />
            </div>
        );
    }

    if (isError || errorValidators) {
        return (
            <div className="mb-2 flex h-full w-full items-center justify-center p-2">
                <InfoBox
                    title="Something went wrong"
                    supportingText={delegatedStakeDataError?.message ?? 'An error occurred'}
                    style={InfoBoxStyle.Default}
                    type={InfoBoxType.Error}
                    icon={<Warning />}
                />
            </div>
        );
    }

    return (
        <DialogLayout>
            <Header title="Unstake" onClose={handleClose} onBack={onBack} titleCentered />
            <DialogLayoutBody>
                <div className="flex flex-col gap-y-md">
                    <Validator
                        address={extendedStake.validatorAddress}
                        isSelected
                        showActiveStatus={showActiveStatus}
                    />

                    <ValidatorStakingData
                        validatorAddress={extendedStake.validatorAddress}
                        stakeId={extendedStake.stakedIotaId}
                        isUnstake
                    />

                    <Panel hasBorder>
                        <div className="flex flex-col gap-y-sm p-md">
                            <div className="flex items-center justify-between">
                                <span className="text-neutral-40 text-label-lg">
                                    Unstake Amount
                                </span>
                                <Button
                                    type={ButtonType.Secondary}
                                    text={isPartialUnstake ? 'Partial Unstake' : 'Unstake All'}
                                    onClick={() => {
                                        setIsPartialUnstake(!isPartialUnstake);
                                        setPartialUnstakeAmount('');
                                    }}
                                />
                            </div>
                            {isPartialUnstake && (
                                <>
                                    <Input
                                        type={InputType.NumericFormat}
                                        value={partialUnstakeAmount}
                                        onChange={(e) => setPartialUnstakeAmount(e.target.value)}
                                        placeholder="Enter amount to unstake"
                                        suffix=" IOTA"
                                        errorMessage={getErrorMessage()}
                                    />
                                    <div className="text-neutral-60 text-body-sm">
                                        Minimum: 1 IOTA to unstake and 1 IOTA must remain staked
                                    </div>
                                </>
                            )}
                        </div>
                    </Panel>

                    <Panel hasBorder>
                        <div className="flex flex-col gap-y-sm p-md">
                            {isPartialUnstake ? (
                                <>
                                    <KeyValueInfo
                                        keyText="Amount to Unstake"
                                        value={unstakeAmountFormatted}
                                        supportingLabel={GAS_SYMBOL}
                                        fullwidth
                                    />
                                    <KeyValueInfo
                                        keyText="Rewards Earned"
                                        value={rewardsFormatted}
                                        supportingLabel={rewardSymbol}
                                        fullwidth
                                    />
                                    <Divider />
                                    <KeyValueInfo
                                        keyText="Remaining Stake"
                                        value={remainingStakeFormatted}
                                        supportingLabel={GAS_SYMBOL}
                                        fullwidth
                                    />
                                    <KeyValueInfo
                                        keyText="Remaining Rewards"
                                        value={remainingRewardsFormatted}
                                        supportingLabel={remainingRewardsSymbol}
                                        fullwidth
                                    />
                                    <Divider />
                                    <KeyValueInfo
                                        keyText="Total Unstaked IOTA"
                                        value={totalUnstakeAmountFormatted}
                                        supportingLabel={GAS_SYMBOL}
                                        fullwidth
                                    />
                                    <KeyValueInfo
                                        keyText="Remaining Total Staked IOTA"
                                        value={remainingTotalStakedFormatted}
                                        supportingLabel={GAS_SYMBOL}
                                        fullwidth
                                    />
                                </>
                            ) : (
                                <>
                                    <KeyValueInfo
                                        keyText="Your Stake"
                                        value={unstakeAmountFormatted}
                                        supportingLabel={GAS_SYMBOL}
                                        fullwidth
                                    />
                                    <KeyValueInfo
                                        keyText="Rewards Earned"
                                        value={rewardsFormatted}
                                        supportingLabel={rewardSymbol}
                                        fullwidth
                                    />
                                    <Divider />
                                    <KeyValueInfo
                                        keyText="Total unstaked IOTA"
                                        value={totalUnstakeAmountFormatted}
                                        supportingLabel={GAS_SYMBOL}
                                        fullwidth
                                    />
                                </>
                            )}
                        </div>
                    </Panel>

                    <Panel hasBorder>
                        <div className="flex flex-col gap-y-sm p-md">
                            <KeyValueInfo
                                keyText="Gas Fees"
                                value={gasFormatted || '-'}
                                supportingLabel={GAS_SYMBOL}
                                fullwidth
                            />
                        </div>
                    </Panel>
                </div>
            </DialogLayoutBody>

            <DialogLayoutFooter>
                {isNotEnoughGas && (
                    <div className="pt-sm">
                        <InfoBox
                            supportingText={GAS_BUDGET_ERROR_MESSAGES[GAS_BALANCE_TOO_LOW_ID]}
                            icon={<Info />}
                            type={InfoBoxType.Error}
                            style={InfoBoxStyle.Elevated}
                        />
                    </div>
                )}
                {getErrorMessage() && !isPartialUnstake && (
                    <div className="pt-sm">
                        <InfoBox
                            supportingText={getErrorMessage()}
                            icon={<Info />}
                            type={InfoBoxType.Error}
                            style={InfoBoxStyle.Elevated}
                        />
                    </div>
                )}
                <Button
                    type={ButtonType.Secondary}
                    fullWidth
                    onClick={handleUnstake}
                    disabled={
                        !activeUnstakeData ||
                        activeIsPending ||
                        isTransactionPending ||
                        isNotEnoughGas ||
                        activeIsError ||
                        isInvalidAmount ||
                        !delegationId
                    }
                    text="Unstake"
                    icon={
                        activeIsPending || isTransactionPending ? (
                            <LoadingIndicator data-testid="loading-indicator" />
                        ) : null
                    }
                    iconAfterText
                />
            </DialogLayoutFooter>
        </DialogLayout>
    );
}
