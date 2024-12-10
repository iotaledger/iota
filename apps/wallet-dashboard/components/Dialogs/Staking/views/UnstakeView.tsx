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
    ExtendedDelegatedStake,
    GAS_SYMBOL,
    useFormatCoin,
    useGetStakingValidatorDetails,
} from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useCurrentAccount } from '@iota/dapp-kit';
import { Warning } from '@iota/ui-icons';
import { StakeRewardsPanel, ValidatorStakingData } from '@/components';
import { DialogLayout, DialogLayoutFooter, DialogLayoutBody } from '../../layout';
import { Transaction } from '@iota/iota-sdk/transactions';
import { Validator } from './Validator';

interface UnstakeDialogProps {
    extendedStake: ExtendedDelegatedStake;
    handleClose: () => void;
    handleUnstake: () => void;
    showActiveStatus?: boolean;
    isUnstakePending: boolean;
    gasBudget: string | number | null | undefined;
    unstakeTx: Transaction | undefined;
    onBack?: () => void;
}

export function UnstakeView({
    extendedStake,
    handleClose,
    onBack,
    handleUnstake,
    isUnstakePending,
    gasBudget,
    unstakeTx,
    showActiveStatus,
}: UnstakeDialogProps): JSX.Element {
    const activeAddress = useCurrentAccount()?.address ?? null;
    const [gasFormatted] = useFormatCoin(gasBudget, IOTA_TYPE_ARG);

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

    const delegationId = extendedStake?.status === 'Active' && extendedStake?.stakedIotaId;

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

    const isUnstakeButtonDisabled = !unstakeTx || isUnstakePending || !delegationId;

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
                    disabled={isUnstakeButtonDisabled}
                    text="Unstake"
                    icon={
                        isUnstakeButtonDisabled ? (
                            <LoadingIndicator data-testid="loading-indicator" />
                        ) : null
                    }
                    iconAfterText
                />
            </DialogLayoutFooter>
        </DialogLayout>
    );
}
