// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Button,
    KeyValueInfo,
    Divider,
    ButtonType,
    Panel,
    LoadingIndicator,
    InfoBoxType,
    InfoBoxStyle,
    InfoBox,
} from '@iota/apps-ui-kit';
import {
    createUnstakeTransaction,
    ExtendedDelegatedStake,
    GAS_SYMBOL,
    TimeUnit,
    useFormatCoin,
    useGetTimeBeforeEpochNumber,
    useGetStakingValidatorDetails,
    useTimeAgo,
    useTransactionGasBudget,
} from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useMemo } from 'react';
import { useCurrentAccount, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { Loader, Warning } from '@iota/ui-icons';
import { useUnstakeTransaction } from '@/hooks';
import { ValidatorStakingData } from '@/components';

interface UnstakeDialogProps {
    extendedStake: ExtendedDelegatedStake;
    handleClose: () => void;
    showActiveStatus?: boolean;
}

export function UnstakeDialogView({
    extendedStake,
    handleClose,
    showActiveStatus,
}: UnstakeDialogProps): JSX.Element {
    const stakingReward = BigInt(extendedStake.estimatedReward ?? '').toString();
    const [rewards, rewardSymbol] = useFormatCoin(stakingReward, IOTA_TYPE_ARG);
    const activeAddress = useCurrentAccount()?.address ?? null;

    const {
        totalStake: [tokenBalance],
        epoch,
        systemDataResult,
        delegatedStakeDataResult,
    } = useGetStakingValidatorDetails({
        accountAddress: activeAddress,
        validatorAddress: extendedStake.validatorAddress,
        stakeId: extendedStake.stakedIotaId,
        unstake: true,
    });

    const { isLoading: loadingValidators, error: errorValidators } = systemDataResult;
    const {
        isLoading: isLoadingDelegatedStakeData,
        isError,
        error: delegatedStakeDataError,
    } = delegatedStakeDataResult;

    const delegationId = extendedStake?.status === 'Active' && extendedStake?.stakedIotaId;

    const [totalIota] = useFormatCoin(BigInt(stakingReward || 0) + tokenBalance, IOTA_TYPE_ARG);

    const transaction = useMemo(
        () => createUnstakeTransaction(extendedStake.stakedIotaId),
        [extendedStake],
    );
    const { data: gasBudget } = useTransactionGasBudget(activeAddress, transaction);

    const { data: currentEpochEndTime } = useGetTimeBeforeEpochNumber(epoch + 1 || 0);
    const currentEpochEndTimeAgo = useTimeAgo({
        timeFrom: currentEpochEndTime,
        endLabel: '--',
        shortedTimeLabel: false,
        shouldEnd: true,
        maxTimeUnit: TimeUnit.ONE_HOUR,
    });

    const { data: unstakeData } = useUnstakeTransaction(
        extendedStake.stakedIotaId,
        activeAddress || '',
    );
    const { mutateAsync: signAndExecuteTransaction, isPending } = useSignAndExecuteTransaction();

    async function handleUnstake(): Promise<void> {
        if (!unstakeData) return;
        await signAndExecuteTransaction({
            transaction: unstakeData.transaction,
        });
        handleClose();
    }

    const currentEpochEndTimeFormatted =
        currentEpochEndTime > 0 ? currentEpochEndTimeAgo : `Epoch #${epoch}`;

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
        <>
            <div className="flex h-full w-full flex-col justify-between">
                <div className="flex flex-col gap-y-md">
                    <ValidatorStakingData
                        validatorAddress={extendedStake.validatorAddress}
                        stakeId={extendedStake.stakedIotaId}
                        isUnstake
                    />

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
                                value={tokenBalance}
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
                                value={gasBudget || '-'}
                                supportingLabel={GAS_SYMBOL}
                                fullwidth
                            />
                        </div>
                    </Panel>
                </div>
            </div>

            <div className="flex w-full gap-2.5">
                <Button
                    type={ButtonType.Secondary}
                    fullWidth
                    onClick={handleUnstake}
                    disabled={isPending || !delegationId}
                    text="Unstake"
                    icon={
                        isPending ? (
                            <Loader className="animate-spin" data-testid="loading-indicator" />
                        ) : null
                    }
                    iconAfterText
                />
            </div>
        </>
    );
}
