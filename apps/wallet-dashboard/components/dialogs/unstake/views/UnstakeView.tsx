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
} from '@iota/apps-ui-kit';
import {
    CoinFormat,
    ExtendedDelegatedStake,
    GAS_SYMBOL,
    useFormatCoin,
    useGetStakingValidatorDetails,
    Validator,
} from '@iota/core';
import { useCurrentAccount, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { Warning } from '@iota/apps-ui-icons';
import { StakeRewardsPanel, ValidatorStakingData } from '@/components';
import { DialogLayout, DialogLayoutFooter, DialogLayoutBody } from '../../layout';

import { useNewUnstakeTransaction } from '@/hooks';
import { IotaSignAndExecuteTransactionOutput } from '@iota/wallet-standard';
import toast from 'react-hot-toast';
import { ampli } from '@/lib/utils/analytics';

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
    const { data: unstakeData, isPending: isUnstakeTxPending } = useNewUnstakeTransaction(
        activeAddress,
        extendedStake.stakedIotaId,
    );
    const [gasFormatted] = useFormatCoin({
        balance: unstakeData?.gasBudget,
        format: CoinFormat.FULL,
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

    const { isLoading: loadingValidators, error: errorValidators } = systemDataResult;
    const {
        isLoading: isLoadingDelegatedStakeData,
        isError,
        error: delegatedStakeDataError,
    } = delegatedStakeDataResult;

    const delegationId = extendedStake?.stakedIotaId;
    const isPreparingUnstake = !unstakeData || isUnstakeTxPending;

    async function handleUnstake(): Promise<void> {
        if (!unstakeData) return;

        await signAndExecuteTransaction(
            {
                transaction: unstakeData.transaction,
            },
            {
                onSuccess: (tx) => {
                    toast.success('Unstake transaction has been sent');
                    onSuccess(tx);

                    ampli.unstakedIota({
                        validatorAddress: extendedStake.validatorAddress,
                    });
                },
            },
        ).catch(() => {
            toast.error('Unstake transaction was not sent');
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

                    <StakeRewardsPanel
                        stakingRewards={extendedStake.estimatedReward}
                        totalStaked={totalStakeOriginal}
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
                <Button
                    type={ButtonType.Secondary}
                    fullWidth
                    onClick={handleUnstake}
                    disabled={isPreparingUnstake || isTransactionPending || !delegationId}
                    text="Unstake"
                    icon={
                        isPreparingUnstake ? (
                            <LoadingIndicator data-testid="loading-indicator" />
                        ) : null
                    }
                    iconAfterText
                />
            </DialogLayoutFooter>
        </DialogLayout>
    );
}
