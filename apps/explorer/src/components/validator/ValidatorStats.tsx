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
import { getValidatorEffectiveCommission, useFormatCoin } from '@iota/core';
import { EpochStatusIndicator } from '~/pages/validator/ValidatorDetails';

type StatsCardProps = {
    validatorData: IotaValidatorSummary;
    epoch: number | string;
    epochRewards: number | null;
    apy: number | string | null;
    isEarningCurrentEpoch: boolean;
};

export function ValidatorStats({
    validatorData,
    apy,
    isEarningCurrentEpoch,
}: StatsCardProps): JSX.Element {
    const totalStake = Number(validatorData.stakingPoolIotaBalance);

    const effectiveCommissionRate = getValidatorEffectiveCommission(validatorData);
    const rewardsPoolBalance = Number(validatorData.rewardsPool);

    const [formattedTotalStakeAmount, totalStakeSymbol] = useFormatCoin({
        balance: totalStake,
    });
    const [formattedRewardsPoolBalance, rewardsPoolBalanceSymbol] = useFormatCoin({
        balance: rewardsPoolBalance,
    });

    const votingPower = Number(validatorData.votingPower) / 100;
    const commission = Number(validatorData.commissionRate) / 100;

    return (
        <Panel>
            <Title
                title="Current Epoch"
                trailingElement={
                    <EpochStatusIndicator
                        active={isEarningCurrentEpoch}
                        activeLabel="Earning rewards"
                        inactiveLabel="Not earning"
                        tooltipText="Whether this validator is in the active committee and earning staking rewards this epoch."
                    />
                }
            />
            <div className="flex flex-col gap-md p-md">
                <div className="grid grid-cols-1 gap-md--rs md:grid-cols-2 lg:grid-cols-3">
                    <DisplayStats
                        label="APY"
                        value={apy === null ? 'N/A' : `${apy}%`}
                        tooltipText="This represents the Annualized Percentage Yield based on the validator's past activities. Keep in mind that this APY may not hold true in the future."
                        tooltipPosition={TooltipPosition.Right}
                        type={DisplayStatsType.Default}
                        size={DisplayStatsSize.Large}
                    />
                    <DisplayStats
                        label="Effective Commission"
                        value={effectiveCommissionRate}
                        supportingLabel={`${commission}%`}
                        tooltipText="The base commission chosen by the validator. Note that the actual commission applied is higher because of the dynamic minimum commission rule (IIP-8)."
                        tooltipPosition={TooltipPosition.Right}
                        type={DisplayStatsType.Default}
                        size={DisplayStatsSize.Large}
                    />
                    <DisplayStats
                        label="Voting Power"
                        value={`${votingPower}%`}
                        tooltipText="Share of total committee voting power held by this validator, proportional to its stake."
                        tooltipPosition={TooltipPosition.Right}
                        type={DisplayStatsType.Default}
                        size={DisplayStatsSize.Large}
                    />
                </div>

                <div className="grid grid-cols-2 gap-md--rs lg:grid-cols-2">
                    <DisplayStats
                        label="Total IOTA Staked"
                        value={formattedTotalStakeAmount}
                        supportingLabel={totalStakeSymbol}
                        tooltipText="The total amount of IOTA staked on the network by validators and delegators to secure the network and earn rewards."
                        tooltipPosition={TooltipPosition.Right}
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
                        type={DisplayStatsType.Default}
                    />
                </div>
            </div>
        </Panel>
    );
}
