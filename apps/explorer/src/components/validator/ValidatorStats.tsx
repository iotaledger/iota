// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaValidatorSummary } from '@iota/iota-sdk/client';
import { LabelText, LabelTextSize, Panel, Title, TooltipPosition } from '@iota/apps-ui-kit';
import { getValidatorCommission, useFormatCoin } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';

type StatsCardProps = {
    validatorData: IotaValidatorSummary;
    epoch: number | string;
    epochRewards: number | null;
    apy: number | string | null;
    tallyingScore: string | null;
};

export function ValidatorStats({
    validatorData,
    epochRewards,
    apy,
    tallyingScore,
}: StatsCardProps): JSX.Element {
    const { data: systemState } = useIotaClientQuery('getLatestIotaSystemState');
    // TODO: Add logic for validator stats https://github.com/iotaledger/iota/issues/2449
    const networkStakingParticipation = 0;
    const votedLastRound = 0;

    const totalStake = Number(validatorData.stakingPoolIotaBalance);

    // Temporarily needed to compute the effectiveCommissionRate until infra exposes it in commissionRate directly
    const hasEffectiveCommissionRate = Number(systemState?.protocolVersion ?? 0) >= 20;
    const effectiveCommissionRate = getValidatorCommission(
        validatorData,
        hasEffectiveCommissionRate,
    );
    const commission = getValidatorCommission(validatorData);
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
                        {hasEffectiveCommissionRate && (
                            <LabelText
                                size={LabelTextSize.Medium}
                                label="Effective Commission Rate"
                                text={effectiveCommissionRate}
                                tooltipText="The share of rewards retained by the validator. This rate includes a protocol-enforced minimum to help maintain network decentralization."
                                tooltipPosition={TooltipPosition.Right}
                            />
                        )}
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
            <Panel>
                <Title title="Network Participation" />
                <div className="grid grid-cols-2 gap-md p-md--rs">
                    <div className="grid grid-rows-1 gap-md">
                        <LabelText
                            size={LabelTextSize.Medium}
                            label="Checkpoint Participation"
                            text={networkStakingParticipation || '--'}
                            tooltipText={
                                !networkStakingParticipation
                                    ? 'Coming soon'
                                    : 'The proportion of checkpoints that this validator has certified to date.'
                            }
                            tooltipPosition={TooltipPosition.Right}
                        />
                        <LabelText
                            size={LabelTextSize.Medium}
                            label="Voted Last Round"
                            text={votedLastRound || '--'}
                            tooltipText={
                                !votedLastRound
                                    ? 'Coming soon'
                                    : 'This validator’s participation in the voting for the most recent round.'
                            }
                            tooltipPosition={TooltipPosition.Right}
                        />
                    </div>
                    <div className="grid grid-rows-1 gap-md">
                        <LabelText
                            size={LabelTextSize.Medium}
                            label="Tallying Score"
                            text={tallyingScore ?? '--'}
                            tooltipText={
                                !tallyingScore
                                    ? 'Coming soon'
                                    : 'A score created by validators to assess each other’s performance during IOTA’s standard operations.'
                            }
                            tooltipPosition={TooltipPosition.Right}
                        />
                    </div>
                </div>
            </Panel>
        </div>
    );
}
