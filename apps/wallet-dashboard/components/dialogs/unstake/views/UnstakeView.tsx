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
    Validator,
    toast,
    GAS_BUDGET_ERROR_MESSAGES,
    GAS_BALANCE_TOO_LOW_ID,
    useUnstakeForm,
} from '@iota/core';
import { CoinFormat } from '@iota/iota-sdk/utils';
import { useCurrentAccount, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { Warning, Info } from '@iota/apps-ui-icons';
import { ValidatorStakingData } from '@/components';
import { DialogLayout, DialogLayoutFooter, DialogLayoutBody } from '../../layout';

import { IotaSignAndExecuteTransactionOutput } from '@iota/wallet-standard';
import { ampli } from '@/lib/utils/analytics';
import { useEffect } from 'react';
import { Field, type FieldProps, FormikProvider } from 'formik';

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

    // Calculate the amount to unstake and proportional rewards
    const principalAmount = BigInt(extendedStake.principal);
    const rewardAmount = BigInt(extendedStake.estimatedReward || 0);

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
        activeUnstakeData,
        activeError,
        activeIsError,
        activeIsLoading: activeIsPending,
        gasFormatted,
        isInvalidPartialAmount,
        isNotEnoughGas,
    } = useUnstakeForm({
        activeAddress,
        stakedIotaId: extendedStake.stakedIotaId,
        principalAmount,
        rewardAmount,
    });

    const { mutateAsync: signAndExecuteTransaction, isPending: isTransactionPending } =
        useSignAndExecuteTransaction();

    const { systemDataResult, delegatedStakeDataResult } = useGetStakingValidatorDetails({
        accountAddress: activeAddress,
        validatorAddress: extendedStake.validatorAddress,
        stakeId: extendedStake.stakedIotaId,
        unstake: true,
    });

    useEffect(() => {
        if (activeError) {
            console.error('[DEBUG]: Unstake Error:', activeError);
        }
    }, [activeError]);

    const { isLoading: loadingValidators, error: errorValidators } = systemDataResult;
    const {
        isLoading: isLoadingDelegatedStakeData,
        isError,
        error: delegatedStakeDataError,
    } = delegatedStakeDataResult;

    const delegationId = extendedStake?.stakedIotaId;

    const validatorName =
        systemDataResult.data?.activeValidators.find(
            (v) => v.iotaAddress === extendedStake.validatorAddress,
        )?.name ?? '';

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
                        stakedAmount: Number(unstakeAmountFormattedPlain),
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
        <FormikProvider value={formik}>
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
                                <div className="flex gap-2">
                                    <Button
                                        type={
                                            isPartialUnstake
                                                ? ButtonType.Outlined
                                                : ButtonType.Secondary
                                        }
                                        text="Unstake All"
                                        onClick={switchToFullUnstake}
                                    />
                                    <Button
                                        type={
                                            isPartialUnstake
                                                ? ButtonType.Secondary
                                                : ButtonType.Outlined
                                        }
                                        text="Partial Unstake"
                                        onClick={switchToPartialUnstake}
                                    />
                                </div>
                                {isPartialUnstake && (
                                    <>
                                        <Field name="amount">
                                            {({
                                                field: { onChange, ...field },
                                                meta,
                                            }: FieldProps) => (
                                                <Input
                                                    {...field}
                                                    type={InputType.NumericFormat}
                                                    onValueChange={(vals) =>
                                                        formik.setFieldValue(
                                                            'amount',
                                                            vals.value,
                                                            true,
                                                        )
                                                    }
                                                    placeholder="Enter amount to unstake"
                                                    suffix=" IOTA"
                                                    errorMessage={
                                                        values.amount && meta.error
                                                            ? meta.error
                                                            : undefined
                                                    }
                                                />
                                            )}
                                        </Field>
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
                            isInvalidPartialAmount ||
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
        </FormikProvider>
    );
}
