// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    TimeUnit,
    useGetTimeBeforeEpochNumber,
    useTimeAgo,
    GAS_SYMBOL,
    useGetDelegatedStake,
    DELEGATED_STAKES_QUERY_STALE_TIME,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    getStakeIotaByIotaId,
    getDelegationDataByStakeId,
    Validator,
    toast,
    GAS_BUDGET_ERROR_MESSAGES,
    GAS_BALANCE_TOO_LOW_ID,
    useUnstakeForm,
} from '@iota/core';
import { useMemo } from 'react';
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
import { Field, type FieldProps, FormikProvider } from 'formik';
import { useMutation } from '@tanstack/react-query';
import * as Sentry from '@sentry/react';
import { ampli } from '_src/shared/analytics/ampli';
import { getSignerOperationErrorMessage } from '../../helpers';
import { Info, Loader } from '@iota/apps-ui-icons';
import { type IotaTransactionBlockResponse, type StakeObject } from '@iota/iota-sdk/client';
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

    // Calculate principal and reward amounts
    const principalAmount = totalTokenBalance;
    const rewardAmount = BigInt(iotaEarned);

    const {
        formik,
        values,
        isPartialUnstake,
        switchToFullUnstake,
        switchToPartialUnstake,
        unstakeAmountFormatted,
        rewardsFormatted,
        rewardSymbol,
        totalUnstakeAmountFormatted,
        remainingStakeFormatted,
        remainingRewardsFormatted,
        remainingRewardsSymbol,
        remainingTotalStakedFormatted,
        unstakeAmountFormattedPlain,
        rewardsFormattedPlain,
        transaction,
        activeIsError,
        activeIsLoading,
        gasFormatted,
        gasSymbol,
        isInvalidPartialAmount,
        isNotEnoughGas,
    } = useUnstakeForm({
        activeAddress,
        stakedIotaId,
        principalAmount,
        rewardAmount,
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
                ampli.unstakedIota({
                    stakedAmount: Number(unstakeAmountFormattedPlain),
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

    const isLoading = isPending || isUnstakeTokenTransactionPending || activeIsLoading;

    return (
        <FormikProvider value={formik}>
            <div className="flex flex-1 flex-col flex-nowrap gap-y-md overflow-auto">
                <Validator address={validatorAddress} type={CardType.Filled} />
                <ValidatorFormDetail validatorAddress={validatorAddress} unstake={true} />
                <Panel hasBorder>
                    <div className="flex flex-col gap-y-sm p-md">
                        <div className="flex gap-2">
                            <Button
                                type={isPartialUnstake ? ButtonType.Outlined : ButtonType.Secondary}
                                text="Unstake All"
                                onClick={switchToFullUnstake}
                            />
                            <Button
                                type={isPartialUnstake ? ButtonType.Secondary : ButtonType.Outlined}
                                text="Partial Unstake"
                                onClick={switchToPartialUnstake}
                            />
                        </div>
                        {isPartialUnstake && (
                            <>
                                <Field name="amount">
                                    {({ field: { onChange, ...field }, meta }: FieldProps) => (
                                        <Input
                                            {...field}
                                            type={InputType.NumericFormat}
                                            onValueChange={(vals) =>
                                                formik.setFieldValue('amount', vals.value, true)
                                            }
                                            placeholder="Enter amount to unstake"
                                            suffix=" IOTA"
                                            errorMessage={
                                                values.amount && meta.error ? meta.error : undefined
                                            }
                                        />
                                    )}
                                </Field>
                                <div className="key-value-key-text-color text-body-sm">
                                    Minimum: 1 IOTA to unstake and 1 IOTA must remain staked
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
                    disabled={activeIsError || isLoading || isInvalidPartialAmount}
                    text="Unstake"
                    icon={
                        isLoading && !activeIsError ? (
                            <Loader className="animate-spin" data-testid="loading-indicator" />
                        ) : null
                    }
                    iconAfterText
                />
            </div>
        </FormikProvider>
    );
}
