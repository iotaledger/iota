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
import { StakeRewardsPanel, ValidatorStakingData } from '@/components';
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
    const unstakeAmountNanos = partialUnstakeAmount
        ? BigInt(Math.floor(parseFloat(partialUnstakeAmount) * Number(NANOS_PER_IOTA)))
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
        unstakeAmountNanos
    );

    // Use partial unstake data if enabled, otherwise use full unstake
    const activeUnstakeData = isPartialUnstake && unstakeAmountNanos > 0n ? partialUnstakeData : unstakeData;
    const activeError = isPartialUnstake ? partialError : error;
    const activeIsError = isPartialUnstake ? isPartialUnstakeError : isUnstakeError;
    const activeIsPending = isPartialUnstake ? isPartialUnstakeTxPending : isUnstakeTxPending;

    const [gasFormatted] = useFormatCoin({
        balance: activeUnstakeData?.gasSummary?.totalGas,
        format: CoinFormat.Full,
    });

    const { mutateAsync: signAndExecuteTransaction, isPending: isTransactionPending } =
        useSignAndExecuteTransaction();

    const { totalStakeOriginal, systemDataResult, delegatedStakeDataResult } =
        useGetStakingValidatorDetails({
            accountAddress: activeAddress,
            validatorAddress: extendedStake.validatorAddress,
            stakeId: extendedStake.stakedIotaId,
            unstake: true,
        });

    // Calculate the amount to unstake and proportional rewards
    const principalAmount = BigInt(extendedStake.principal);
    const rewardAmount = BigInt(extendedStake.estimatedReward || 0);
    const totalStaked = principalAmount + rewardAmount;

    const unstakeAmount = isPartialUnstake && unstakeAmountNanos > 0n
        ? unstakeAmountNanos
        : principalAmount;

    // Calculate proportional rewards for partial unstake
    const proportionalRewards = principalAmount > 0n
        ? (rewardAmount * unstakeAmount) / principalAmount
        : 0n;

    const totalUnstakeAmount = unstakeAmount + proportionalRewards;

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
    const isInvalidAmount = isPartialUnstake && (
        unstakeAmountNanos <= 0n ||
        unstakeAmountNanos > principalAmount
    );

    const validatorName =
        systemDataResult.data?.activeValidators.find(
            (v) => v.iotaAddress === extendedStake.validatorAddress,
        )?.name ?? '';

    const [stakedFormattedPlain] = useFormatCoin({
        balance: totalStakeOriginal,
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

                    ampli.unstakedIota({
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
                                <span className="text-label-lg text-neutral-40">Unstake Amount</span>
                                <Button
                                    type={ButtonType.Ghost}
                                    text={isPartialUnstake ? "Unstake All" : "Partial Unstake"}
                                    onClick={() => {
                                        setIsPartialUnstake(!isPartialUnstake);
                                        setPartialUnstakeAmount('');
                                    }}
                                />
                            </div>
                            {isPartialUnstake && (
                                <Input
                                    type={InputType.NumericFormat}
                                    value={partialUnstakeAmount}
                                    onChange={(e) => setPartialUnstakeAmount(e.target.value)}
                                    placeholder="Enter amount to unstake"
                                    suffix=" IOTA"
                                    errorMessage={
                                        isInvalidAmount
                                            ? `Amount must be greater than 0 and at most ${maxUnstakeAmount.toFixed(2)} IOTA`
                                            : undefined
                                    }
                                />
                            )}
                        </div>
                    </Panel>

                    <StakeRewardsPanel
                        stakingRewards={proportionalRewards.toString()}
                        totalStaked={unstakeAmount}
                    />

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
                <Button
                    type={ButtonType.Secondary}
                    fullWidth
                    onClick={handleUnstake}
                    disabled={
                        !activeUnstakeData ||
                        activeIsPending ||
                        isTransactionPending ||
                        isNotEnoughGas ||
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
