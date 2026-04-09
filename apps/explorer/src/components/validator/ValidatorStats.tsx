// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaValidatorSummary } from '@iota/iota-sdk/client';
import {
    DisplayStats,
    DisplayStatsSize,
    DisplayStatsType,
    Panel,
    Title,
    TooltipPosition,
} from '@iota/apps-ui-kit';
import {
    formatPercentageDisplay,
    getValidatorEffectiveCommission,
    useFormatCoin,
} from '@iota/core';
import { CoinFormat } from '@iota/iota-sdk/utils';

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

    const [formattedTotalStakeAmount, totalStakeSymbol] = useFormatCoin({
        balance: totalStake,
        format: CoinFormat.Full,
    });
    const [formattedRewardsPoolBalance, rewardsPoolBalanceSymbol] = useFormatCoin({
        balance: rewardsPoolBalance,
        format: CoinFormat.Full,
    });

    return (
        <Panel>
            <Title title="Validator Stats" />
            <div className="flex flex-col gap-md p-md">
                <div className="grid grid-cols-1 gap-md--rs sm:grid-cols-2">
                    <DisplayStats
                        label="Total IOTA Staked"
                        value={formattedTotalStakeAmount}
                        supportingLabel={totalStakeSymbol}
                        tooltipText="The total amount of IOTA staked on the network by validators and delegators to secure the network and earn rewards."
                        tooltipPosition={TooltipPosition.Right}
                        type={DisplayStatsType.Secondary}
                        size={DisplayStatsSize.Large}
                    />
                    <DisplayStats
                        label="Reward Balance"
                        value={formattedRewardsPoolBalance}
                        supportingLabel={rewardsPoolBalanceSymbol}
                        tooltipText={
                            Number(rewardsPoolBalance) <= 0
                                ? 'Coming soon'
                                : 'Accumulated staking rewards that are currently available to withdraw.'
                        }
                        tooltipPosition={TooltipPosition.Right}
                        type={DisplayStatsType.Secondary}
                        size={DisplayStatsSize.Large}
                    />
                </div>

                <div className="grid grid-cols-2 gap-md--rs md:grid-cols-3">
                    <DisplayStats
                        label="Staking APY"
                        value={apy === null ? 'N/A' : `${apy}%`}
                        tooltipText="This represents the Annualized Percentage Yield based on the validator's past activities. Keep in mind that this APY may not hold true in the future."
                        tooltipPosition={TooltipPosition.Right}
                        type={DisplayStatsType.Default}
                        size={DisplayStatsSize.Default}
                    />
                    <DisplayStats
                        label="Commission"
                        value={commission}
                        tooltipText="The fee this validator charges on staking rewards. May be overridden by voting power (see Effective Commission)."
                        tooltipPosition={TooltipPosition.Right}
                        type={DisplayStatsType.Default}
                        size={DisplayStatsSize.Default}
                    />
                    <DisplayStats
                        label="Effective Commission"
                        value={effectiveCommissionRate}
                        tooltipText="Since protocol v20 (IIP-8), validators with high voting power cannot set an artificially low commission."
                        tooltipPosition={TooltipPosition.Right}
                        type={DisplayStatsType.Default}
                        size={DisplayStatsSize.Default}
                    />
                    {/* <DisplayStats
                        label="Last Epoch Rewards"
                        value={typeof epochRewards === 'number' ? formattedEpochRewards : '0'}
                        supportingLabel={epochRewardsSymbol}
                        tooltipText={
                            epochRewards === null
                                ? 'Coming soon'
                                : 'The staking rewards earned during the previous epoch.'
                        }
                        tooltipPosition={TooltipPosition.Right}
                        type={DisplayStatsType.Default}
                        size={DisplayStatsSize.Default}
                    /> */}
                </div>
            </div>
        </Panel>
    );
}
