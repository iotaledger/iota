// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaValidatorSummary } from '@iota/iota-sdk/client';
import { LabelText, LabelTextSize, Panel, Title, TooltipPosition } from '@iota/apps-ui-kit';
import {
    EFFECTIVE_COMMISSION_TOOLTIP,
    formatPercentageDisplay,
    getValidatorEffectiveCommission,
    useFormatCoin,
} from '@iota/core';

type StatsCardProps = {
    validatorData: IotaValidatorSummary;
    epoch: number | string;
    epochRewards: number | null;
    apy: number | string | null;
};

export function ValidatorStats({ validatorData, epochRewards, apy }: StatsCardProps): JSX.Element {
    const totalStake = Number(validatorData.stakingPoolIotaBalance);

    const effectiveCommissionRate = getValidatorEffectiveCommission(validatorData);
    const commission = formatPercentageDisplay(Number(validatorData.commissionRate) / 100, '--');
    const rewardsPoolBalance = Number(validatorData.rewardsPool);

    const [formattedTotalStakeAmount, totalStakeSymbol] = useFormatCoin({ balance: totalStake });
    const [formattedEpochRewards, epochRewardsSymbol] = useFormatCoin({ balance: epochRewards });
    const [formattedRewardsPoolBalance, rewardsPoolBalanceSymbol] = useFormatCoin({
        balance: rewardsPoolBalance,
    });

    return (
        <div className="flex flex-col gap-lg md:flex-row">
            <Panel>
                <Title title="Staked on Validator" />
                <div className="grid grid-cols-2 gap-md p-md--rs">
                    <div className="grid grid-rows-1 gap-md">
                        <LabelText
                            size={LabelTextSize.Medium}
                            label="Staking APY"
                            text={apy === null ? 'N/A' : `${apy}%`}
                            tooltipText="This represents the Annualized Percentage Yield based on a specific validator's past activities. Keep in mind that this APY may not hold true in the future."
                            tooltipPosition={TooltipPosition.Right}
                        />
                        <LabelText
                            size={LabelTextSize.Medium}
                            label="Total IOTA Staked"
                            text={formattedTotalStakeAmount}
                            supportingLabel={totalStakeSymbol}
                            tooltipText="The total amount of IOTA staked on the network by validators and delegators to secure the network and earn rewards."
                            tooltipPosition={TooltipPosition.Right}
                        />
                    </div>
                    <div className="grid grid-rows-1 gap-md">
                        <LabelText
                            size={LabelTextSize.Medium}
                            label="Effective Commission Rate"
                            text={effectiveCommissionRate}
                            tooltipText={EFFECTIVE_COMMISSION_TOOLTIP}
                            tooltipPosition={TooltipPosition.Right}
                        />
                        <LabelText
                            size={LabelTextSize.Medium}
                            label="Commission"
                            text={commission}
                            tooltipText="The charge imposed by the validator for their staking services."
                            tooltipPosition={TooltipPosition.Right}
                        />
                    </div>
                </div>
            </Panel>
            <Panel>
                <Title title="Validator Staking Rewards" />
                <div className="grid grid-cols-2 gap-md p-md--rs">
                    <LabelText
                        size={LabelTextSize.Medium}
                        label="Last Epoch Rewards"
                        text={typeof epochRewards === 'number' ? formattedEpochRewards : '0'}
                        supportingLabel={epochRewardsSymbol}
                        tooltipText={
                            epochRewards === null
                                ? 'Coming soon'
                                : 'The staking rewards earned during the previous epoch.'
                        }
                        tooltipPosition={TooltipPosition.Right}
                    />
                    <LabelText
                        size={LabelTextSize.Medium}
                        label="Reward Pool"
                        text={formattedRewardsPoolBalance}
                        supportingLabel={rewardsPoolBalanceSymbol}
                        tooltipText={
                            Number(rewardsPoolBalance) <= 0
                                ? 'Coming soon'
                                : 'The current balance in this validator’s reward pool.'
                        }
                        tooltipPosition={TooltipPosition.Right}
                    />
                </div>
            </Panel>
        </div>
    );
}
