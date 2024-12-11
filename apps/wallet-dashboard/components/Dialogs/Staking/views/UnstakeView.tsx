// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Header,
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
import { useCurrentAccount } from '@iota/dapp-kit';
import { Loader, Warning } from '@iota/ui-icons';
import { useUnstakeTransaction } from '@/hooks';
import { ValidatorStakingData } from '@/components';
import { DialogLayout, DialogLayoutFooter, DialogLayoutBody } from '../../layout';
import { Transaction } from '@iota/iota-sdk/transactions';

interface UnstakeDialogProps {
    extendedStake: ExtendedDelegatedStake;
    handleClose: () => void;
    onUnstake: (unstakeTransaction: Transaction) => void;
    isPending: boolean;
}

export function UnstakeView({
    extendedStake,
    handleClose,
    onUnstake,
    isPending,
}: UnstakeDialogProps): JSX.Element {
    const stakingReward = BigInt(extendedStake.estimatedReward ?? '').toString();
    const [rewards, rewardSymbol] = useFormatCoin(stakingReward, IOTA_TYPE_ARG);
    const activeAddress = useCurrentAccount()?.address ?? null;

    const {
        totalStake: [tokenBalance],
        totalStakeOriginal,
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

    const delegationId = extendedStake?.stakedIotaId;

    const [totalIota] = useFormatCoin(
        BigInt(stakingReward || 0) + totalStakeOriginal,
        IOTA_TYPE_ARG,
    );

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

    function handleUnstake(): void {
        if (!unstakeData) return;
        onUnstake(unstakeData.transaction);
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
        <DialogLayout>
            <Header title="Unstake" onClose={handleClose} onBack={handleClose} titleCentered />
            <DialogLayoutBody>
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
            </DialogLayoutBody>

            <DialogLayoutFooter>
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
            </DialogLayoutFooter>
        </DialogLayout>
    );
}
