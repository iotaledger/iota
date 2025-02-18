// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import {
    formatPercentageDisplay,
    useValidatorInfo,
    useGetStakingValidatorDetails,
} from '@iota/core';
import { KeyValueInfo, Panel, TooltipPosition } from '@iota/apps-ui-kit';

interface StakedInfoProps {
    validatorAddress: string;
    accountAddress: string;
}

export function StakedInfo({ validatorAddress, accountAddress }: StakedInfoProps) {
    const { apy, isApyApproxZero } = useValidatorInfo({
        validatorAddress: validatorAddress,
    });

    const { totalValidatorsStake, totalStakePercentage, totalStake, commission } =
        useGetStakingValidatorDetails({
            accountAddress: accountAddress,
            stakeId: null,
            validatorAddress: validatorAddress,
            unstake: false,
        });

    const [totalValidatorStakeFormatted, totalValidatorStakeSymbol] = totalValidatorsStake;
    const [totalStakeFormatted, totalStakeSymbol] = totalStake;

    return (
        <Panel hasBorder>
            <div className="flex flex-col gap-y-sm p-md">
                <KeyValueInfo
                    keyText="Staking APY"
                    tooltipPosition={TooltipPosition.Right}
                    tooltipText="Annualized percentage yield based on past validator performance. Future APY may vary"
                    value={formatPercentageDisplay(apy, '--', isApyApproxZero)}
                    fullwidth
                />
                <KeyValueInfo
                    keyText="Commission"
                    value={commission}
                    fullwidth
                    tooltipText="The charge imposed by the validator for their staking services."
                    tooltipPosition={TooltipPosition.Right}
                />
                <KeyValueInfo
                    keyText="Stake Share"
                    tooltipPosition={TooltipPosition.Right}
                    tooltipText="Stake percentage managed by this validator."
                    value={formatPercentageDisplay(totalStakePercentage)}
                    fullwidth
                />
                <KeyValueInfo
                    keyText="Total Staked"
                    tooltipPosition={TooltipPosition.Right}
                    tooltipText="The full amount of IOTA staked by this validator and delegators for network validation and rewards."
                    value={totalValidatorStakeFormatted}
                    supportingLabel={totalValidatorStakeSymbol}
                    fullwidth
                />
                <KeyValueInfo
                    keyText="Your Staked IOTA"
                    tooltipPosition={TooltipPosition.Right}
                    tooltipText="Your current staked balance."
                    value={totalStakeFormatted}
                    supportingLabel={totalStakeSymbol}
                    fullwidth
                />
            </div>
        </Panel>
    );
}
