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
    ExtendedDelegatedStake,
    GAS_SYMBOL,
    TimeUnit,
    useFormatCoin,
    useGetTimeBeforeEpochNumber,
    useGetStakingValidatorDetails,
    useTimeAgo,
} from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useCurrentAccount } from '@iota/dapp-kit';
import { Warning } from '@iota/ui-icons';
import { ValidatorStakingData } from '@/components';
import { DialogLayout, DialogLayoutFooter, DialogLayoutBody } from '../../layout';
import { Transaction } from '@iota/iota-sdk/transactions';

interface UnstakeDialogProps {
    extendedStake: ExtendedDelegatedStake;
    handleClose: () => void;
    handleUnstake: () => void;
    showActiveStatus?: boolean;
    isUnstakePending: boolean;
    gasBudget: string | number | null | undefined;
    unstakeTx: Transaction | undefined;
}

export function UnstakeView({
    extendedStake,
    handleClose,
    handleUnstake,
    isUnstakePending,
    gasBudget,
    unstakeTx,
    showActiveStatus,
}: UnstakeDialogProps): JSX.Element {
    const stakingReward = BigInt(extendedStake.estimatedReward ?? '').toString();
    const [rewards, rewardSymbol] = useFormatCoin(stakingReward, IOTA_TYPE_ARG);
    const activeAddress = useCurrentAccount()?.address ?? null;
    const [gasFormatted] = useFormatCoin(gasBudget, IOTA_TYPE_ARG);

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

    const delegationId = extendedStake?.status === 'Active' && extendedStake?.stakedIotaId;

    const [totalIota] = useFormatCoin(
        BigInt(stakingReward || 0) + totalStakeOriginal,
        IOTA_TYPE_ARG,
    );

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
                                value={gasFormatted || '-'}
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
                    disabled={!unstakeTx || isUnstakePending || !delegationId}
                    text="Unstake"
                    icon={
                        unstakeTx || isUnstakePending ? (
                            <LoadingIndicator data-testid="loading-indicator" />
                        ) : null
                    }
                    iconAfterText
                />
            </DialogLayoutFooter>
        </DialogLayout>
    );
}
