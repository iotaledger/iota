// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { formatPercentageDisplay, useGetStakingValidatorDetails } from '@iota/core';
import { useSearchParams } from 'react-router-dom';
import { useActiveAddress } from '_hooks';
import {
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    KeyValueInfo,
    Panel,
    TooltipPosition,
    LoadingIndicator,
} from '@iota/apps-ui-kit';
import { Warning } from '@iota/apps-ui-icons';

interface ValidatorFormDetailProps {
    validatorAddress: string;
    unstake?: boolean;
}

export function ValidatorFormDetail({ validatorAddress, unstake }: ValidatorFormDetailProps) {
    const accountAddress = useActiveAddress();
    const [searchParams] = useSearchParams();
    const stakeIdParams = searchParams.get('staked');

    const {
        totalStakePercentage,
        validatorApy: { apy, isApyApproxZero },
        totalValidatorsStake: [totalValidatorStakeFormatted, totalValidatorStakeSymbol],
        totalStake: [totalStakeFormatted, totalStakeSymbol],
        delegatedStakeDataResult,
        systemDataResult,
    } = useGetStakingValidatorDetails({
        accountAddress,
        validatorAddress,
        stakeId: stakeIdParams,
        unstake,
    });

    const {
        isLoading: isLoadingDelegatedStake,
        isError: isDelegatedStakeErrored,
        error: delegatedStakeError,
    } = delegatedStakeDataResult;
    const { isLoading: isLoadingSystemData, isError: isSystemDataErrored } = systemDataResult;

    if (isLoadingDelegatedStake || isLoadingSystemData) {
        return (
            <div className="flex h-full w-full items-center justify-center p-2">
                <LoadingIndicator />
            </div>
        );
    }

    if (isDelegatedStakeErrored || isSystemDataErrored) {
        return (
            <InfoBox
                type={InfoBoxType.Error}
                title={delegatedStakeError?.message ?? 'Error loading validator data'}
                icon={<Warning />}
                style={InfoBoxStyle.Elevated}
            />
        );
    }

    return (
        <div className="w-full">
            <Panel hasBorder>
                <div className="flex flex-col gap-y-sm p-md">
                    <KeyValueInfo
                        keyText="Staking APY"
                        tooltipPosition={TooltipPosition.Bottom}
                        tooltipText="Annualized percentage yield based on past validator performance. Future APY may vary"
                        value={formatPercentageDisplay(apy, '--', isApyApproxZero)}
                        fullwidth
                    />
                    <KeyValueInfo
                        keyText="Stake Share"
                        tooltipPosition={TooltipPosition.Bottom}
                        tooltipText="Stake percentage managed by this validator."
                        value={formatPercentageDisplay(totalStakePercentage)}
                        fullwidth
                    />
                    {!unstake && (
                        <>
                            <KeyValueInfo
                                keyText="Total Staked"
                                tooltipPosition={TooltipPosition.Bottom}
                                tooltipText="The full amount of IOTA staked by this validator and delegators for network validation and rewards."
                                value={totalValidatorStakeFormatted}
                                supportingLabel={totalValidatorStakeSymbol}
                                fullwidth
                            />
                            <KeyValueInfo
                                keyText="Your Staked IOTA"
                                tooltipPosition={TooltipPosition.Bottom}
                                tooltipText="Your current staked balance."
                                value={totalStakeFormatted}
                                supportingLabel={totalStakeSymbol}
                                fullwidth
                            />
                        </>
                    )}
                </div>
            </Panel>
        </div>
    );
}
