// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    TimeUnit,
    useFormatCoin,
    useGetTimeBeforeEpochNumber,
    useTimeAgo,
    GAS_SYMBOL,
    useNewUnstakeTransaction,
    useNewPartialUnstakeTransaction,
    useGetDelegatedStake,
    DELEGATED_STAKES_QUERY_STALE_TIME,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    getStakeIotaByIotaId,
    getDelegationDataByStakeId,
    Validator,
    toast,
    GAS_BUDGET_ERROR_MESSAGES,
    NOT_ENOUGH_BALANCE_ID,
    GAS_BALANCE_TOO_LOW_ID,
} from '@iota/core';
import { useMemo, useState } from 'react';
import { useActiveAccount, useSigner } from '_hooks';
import { useIotaClientQuery } from '@iota/dapp-kit';
import {
    Button,
    ButtonType,
    CardType,
    Divider,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    KeyValueInfo,
    Panel,
    Input,
    InputType,
} from '@iota/apps-ui-kit';
import { useMutation } from '@tanstack/react-query';
import * as Sentry from '@sentry/react';
import { ampli } from '_src/shared/analytics/ampli';
import { getSignerOperationErrorMessage } from '../../helpers';
import { Info, Loader } from '@iota/apps-ui-icons';
import { type IotaTransactionBlockResponse, type StakeObject } from '@iota/iota-sdk/client';
import { CoinFormat, NANOS_PER_IOTA } from '@iota/iota-sdk/utils';
import { ValidatorFormDetail } from './ValidatorFormDetail';

export interface StakeFromProps {
    stakedIotaId: string;
    validatorAddress: string;
    epoch: number;
    onSuccess: (response: IotaTransactionBlockResponse) => void;
}

export function UnStakeForm({ stakedIotaId, validatorAddress, epoch, onSuccess }: StakeFromProps) {
    const activeAccount = useActiveAccount();
    const activeAddress = activeAccount?.address ?? '';
    const signer = useSigner(activeAccount);
    const { data: systemState } = useIotaClientQuery('getLatestIotaSystemState');
    const validatorName =
        systemState?.activeValidators.find((v) => v.iotaAddress === validatorAddress)?.name ?? '';
    const [partialUnstakeAmount, setPartialUnstakeAmount] = useState<string>('');
    const [isPartialUnstake, setIsPartialUnstake] = useState(false);

    const { data: allDelegation, isPending } = useGetDelegatedStake({
        address: activeAddress || '',
        staleTime: DELEGATED_STAKES_QUERY_STALE_TIME,
        refetchInterval: DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    });

    const totalTokenBalance = useMemo(() => {
        if (!allDelegation) return 0n;
        // return only the total amount of tokens staked for a specific stakeId
        return getStakeIotaByIotaId(allDelegation, stakedIotaId);
    }, [allDelegation, stakedIotaId]);

    const stakeData = useMemo(() => {
        if (!allDelegation || !stakedIotaId) return null;
        // return delegation data for a specific stakeId
        return getDelegationDataByStakeId(allDelegation, stakedIotaId);
    }, [allDelegation, stakedIotaId]);

    const iotaEarned =
        (stakeData as Extract<StakeObject, { estimatedReward: string }>)?.estimatedReward || '0';

    // Parse the unstake amount in nanos
    const parsedAmount = parseFloat(partialUnstakeAmount);
    const unstakeAmountNanos = partialUnstakeAmount && !isNaN(parsedAmount) && parsedAmount > 0
        ? BigInt(Math.floor(parsedAmount * Number(NANOS_PER_IOTA)))
        : 0n;

    // Calculate principal and reward amounts
    const principalAmount = totalTokenBalance;
    const rewardAmount = BigInt(iotaEarned);
    const totalStaked = principalAmount + rewardAmount;

    const unstakeAmount = isPartialUnstake && unstakeAmountNanos > 0n
        ? unstakeAmountNanos
        : principalAmount;

    // Calculate proportional rewards for partial unstake
    const proportionalRewards = principalAmount > 0n
        ? (rewardAmount * unstakeAmount) / principalAmount
        : 0n;

    const [rewards, rewardSymbol] = useFormatCoin({ balance: proportionalRewards });
    const [totalIota] = useFormatCoin({ balance: unstakeAmount + proportionalRewards });
    const [tokenBalanceFormatted] = useFormatCoin({ balance: unstakeAmount });
    const [tokenBalanceFormattedPlain] = useFormatCoin({
        balance: unstakeAmount,
        format: CoinFormat.Full,
        useGroupSeparator: false,
    });
    const [rewardsFormattedPlain] = useFormatCoin({
        balance: proportionalRewards,
        format: CoinFormat.Full,
        useGroupSeparator: false,
    });

    const {
        data: unstakeData,
        isLoading: isUnstakeTokenTransactionLoading,
        isError,
        error,
    } = useNewUnstakeTransaction(activeAddress, stakedIotaId);

    const {
        data: partialUnstakeData,
        isLoading: isPartialUnstakeTokenTransactionLoading,
        isError: isPartialError,
        error: partialError,
    } = useNewPartialUnstakeTransaction(
        activeAddress,
        stakedIotaId,
        unstakeAmountNanos
    );

    // Use partial unstake data if enabled, otherwise use full unstake
    const activeUnstakeData = isPartialUnstake && unstakeAmountNanos > 0n ? partialUnstakeData : unstakeData;
    const activeError = isPartialUnstake ? partialError : error;
    const activeIsError = isPartialUnstake ? isPartialError : isError;
    const activeIsLoading = isPartialUnstake ? isPartialUnstakeTokenTransactionLoading : isUnstakeTokenTransactionLoading;

    const transaction = activeUnstakeData?.transaction;

    const [formattedGas, gasSymbol] = useFormatCoin({
        balance: activeUnstakeData?.gasSummary?.totalGas,
        format: CoinFormat.Full,
    });
    const { data: currentEpochEndTime } = useGetTimeBeforeEpochNumber(epoch + 1 || 0);
    const currentEpochEndTimeAgo = useTimeAgo({
        timeFrom: currentEpochEndTime,
        endLabel: '--',
        shortedTimeLabel: false,
        shouldEnd: true,
        maxTimeUnit: TimeUnit.ONE_HOUR,
    });

    const currentEpochEndTimeFormatted =
        currentEpochEndTime > 0 ? currentEpochEndTimeAgo : `Epoch #${epoch}`;

    const maxUnstakeAmount = Number(principalAmount) / Number(NANOS_PER_IOTA);
    const MIN_STAKING_THRESHOLD = 1_000_000_000n; // 1 IOTA in nanos

    // For partial unstake:
    // - Unstake amount must be >= MIN_STAKING_THRESHOLD
    // - Remaining amount must be >= MIN_STAKING_THRESHOLD
    // - OR user must unstake everything (full unstake)
    const remainingAmount = principalAmount - unstakeAmountNanos;
    const isInvalidAmount = isPartialUnstake && (
        unstakeAmountNanos <= 0n ||
        unstakeAmountNanos > principalAmount ||
        (unstakeAmountNanos < MIN_STAKING_THRESHOLD && unstakeAmountNanos !== principalAmount) ||
        (remainingAmount > 0n && remainingAmount < MIN_STAKING_THRESHOLD)
    );

    // Determine the appropriate error message
    const getErrorMessage = () => {
        if (!isPartialUnstake || !isInvalidAmount) return undefined;

        if (unstakeAmountNanos <= 0n) {
            return 'Amount must be greater than 0';
        }
        if (unstakeAmountNanos > principalAmount) {
            return `Amount cannot exceed ${maxUnstakeAmount.toFixed(2)} IOTA`;
        }
        if (unstakeAmountNanos < MIN_STAKING_THRESHOLD) {
            return 'Unstake amount must be at least 1 IOTA';
        }
        if (remainingAmount > 0n && remainingAmount < MIN_STAKING_THRESHOLD) {
            return 'Remaining stake must be at least 1 IOTA or unstake all';
        }
        return undefined;
    };

    const { mutateAsync: unStakeTokenMutateAsync, isPending: isUnstakeTokenTransactionPending } =
        useMutation({
            mutationFn: async () => {
                if (!transaction || !signer) {
                    throw new Error('Failed, missing required field.');
                }

                return Sentry.startSpan(
                    {
                        name: 'unstake',
                    },
                    async (span) => {
                        try {
                            const tx = await signer.signAndExecuteTransaction({
                                transactionBlock: transaction,
                                options: {
                                    showInput: true,
                                    showEffects: true,
                                    showEvents: true,
                                },
                            });
                            await signer.client.waitForTransaction({
                                digest: tx.digest,
                            });
                            return tx;
                        } finally {
                            span?.end();
                        }
                    },
                );
            },
            onSuccess: () => {
                ampli.iotaUnstaked({
                    stakedAmount: Number(tokenBalanceFormattedPlain),
                    validatorAddress: validatorAddress!,
                    rewards: Number(rewardsFormattedPlain),
                    validatorName,
                });
            },
        });
    const handleSubmit = async () => {
        try {
            const response = await unStakeTokenMutateAsync();
            onSuccess(response);
        } catch (error) {
            toast.error(
                <div className="flex max-w-xs flex-col overflow-hidden">
                    <strong>Unstake failed</strong>
                    <small className="overflow-hidden text-ellipsis">
                        {getSignerOperationErrorMessage(error)}
                    </small>
                </div>,
            );
        }
    };

    const isLoading =
        isPending || isUnstakeTokenTransactionPending || activeIsLoading;

    const isNotEnoughGas =
        activeError &&
        (activeError.message.includes(NOT_ENOUGH_BALANCE_ID) ||
            activeError.message.includes(GAS_BALANCE_TOO_LOW_ID));
    return (
        <>
            <div className="flex flex-1 flex-col flex-nowrap gap-y-md overflow-auto">
                <Validator address={validatorAddress} type={CardType.Filled} />
                <ValidatorFormDetail validatorAddress={validatorAddress} unstake={true} />
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
                            <>
                                <Input
                                    type={InputType.NumericFormat}
                                    value={partialUnstakeAmount}
                                    onChange={(e) => setPartialUnstakeAmount(e.target.value)}
                                    placeholder="Enter amount to unstake"
                                    suffix=" IOTA"
                                    errorMessage={getErrorMessage()}
                                />
                                <div className="text-body-sm text-neutral-60">
                                    Minimum: 1 IOTA to unstake and 1 IOTA must remain staked (or unstake all)
                                </div>
                            </>
                        )}
                    </div>
                </Panel>
                <Panel hasBorder>
                    <div className="flex flex-col gap-y-sm p-md">
                        <KeyValueInfo
                            keyText="Current Epoch Ends"
                            value={currentEpochEndTimeFormatted}
                            fullwidth
                        />
                        <Divider />
                        <KeyValueInfo
                            keyText="Your Stake"
                            value={tokenBalanceFormatted}
                            supportingLabel={GAS_SYMBOL}
                            fullwidth
                        />
                        <KeyValueInfo
                            keyText="Rewards Earned"
                            value={rewards}
                            supportingLabel={rewardSymbol}
                            fullwidth
                        />
                        <Divider />
                        <KeyValueInfo
                            keyText="Total unstaked IOTA"
                            value={totalIota}
                            supportingLabel={GAS_SYMBOL}
                            fullwidth
                        />
                    </div>
                </Panel>
                <Panel hasBorder>
                    <div className="flex flex-col gap-y-sm p-md">
                        <KeyValueInfo
                            keyText="Gas Fees"
                            value={formattedGas || '-'}
                            supportingLabel={gasSymbol}
                            fullwidth
                        />
                    </div>
                </Panel>
            </div>
            {Number(iotaEarned) == 0 && (
                <div className="pt-sm">
                    <InfoBox
                        supportingText="You have not earned any rewards yet"
                        icon={<Info />}
                        type={InfoBoxType.Default}
                        style={InfoBoxStyle.Elevated}
                    />
                </div>
            )}
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
            <div className="pt-sm">
                <Button
                    type={ButtonType.Primary}
                    fullWidth
                    onClick={handleSubmit}
                    disabled={activeIsError || isLoading || isInvalidAmount}
                    text="Unstake"
                    icon={
                        isLoading && !activeIsError ? (
                            <Loader className="animate-spin" data-testid="loading-indicator" />
                        ) : null
                    }
                    iconAfterText
                />
            </div>
        </>
    );
}
