// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaValidatorSummary } from '@iota/iota-sdk/client';
import { LabelText, LabelTextSize, Panel, Title, TooltipPosition } from '@iota/apps-ui-kit';
import { CoinFormat, formatBalance, useFormatCoin } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

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
    // TODO: add missing fields
    // const numberOfDelegators = 0;
    //  const networkStakingParticipation = 0;
    //  const votedLastRound =  0;
    //  const lastNarwhalRound = 0;

    const totalStake = Number(validatorData.stakingPoolIotaBalance);
    const commission = Number(validatorData.commissionRate) / 100;
    const rewardsPoolBalance = Number(validatorData.rewardsPool);

    const [formattedTotalStakeAmount, totalStakeSymbol] = useFormatCoin(totalStake, IOTA_TYPE_ARG);
    const [formattedEpochRewards, epochRewardsSymbol] = useFormatCoin(epochRewards, IOTA_TYPE_ARG);
    const [formattedRewardsPoolBalance, rewardsPoolBalanceSymbol] = useFormatCoin(
        rewardsPoolBalance,
        IOTA_TYPE_ARG,
    );
    const nextEpochGasPriceAmount = formatBalance(
        validatorData.nextEpochGasPrice,
        0,
        CoinFormat.FULL,
    );
    return (
        <div className="flex flex-col gap-lg md:flex-row">
            <Panel>
                <Title title="Staked on Validator" />
                <div className="flex flex-col gap-lg p-md--rs">
                    <div className="flex w-full flex-row justify-between gap-md">
                        <LabelText
                            size={LabelTextSize.Medium}
                            label="Staking APY"
                            text={apy === null ? 'N/A' : `${apy}%`}
                            showSupportingLabel={false}
                            tooltipText="This represents the Annualized Percentage Yield based on a specific validator's past activities. Keep in mind that this APY may not hold true in the future."
                            tooltipPosition={TooltipPosition.Right}
                        />
                        <LabelText
                            size={LabelTextSize.Medium}
                            label="Total IOTA Staked"
                            text={formattedTotalStakeAmount}
                            supportingLabel={totalStakeSymbol}
                            showSupportingLabel
                            tooltipText="The total amount of IOTA staked on the network by validators and delegators to secure the network and earn rewards."
                            tooltipPosition={TooltipPosition.Right}
                        />
                    </div>
                    <div className="flex w-full flex-row justify-between gap-md">
                        <LabelText
                            size={LabelTextSize.Medium}
                            label="Commission"
                            text={`${commission}%`}
                            showSupportingLabel={false}
                            tooltipText="The charge imposed by the validator for their staking services."
                            tooltipPosition={TooltipPosition.Right}
                        />
                        <LabelText
                            size={LabelTextSize.Medium}
                            label="Delegators"
                            text="--"
                            showSupportingLabel={false}
                            tooltipText="Coming soon"
                            tooltipPosition={TooltipPosition.Right}
                        />
                    </div>
                </div>
            </Panel>
            <Panel>
                <Title title="Validator Staking Rewards" />
                <div className="flex flex-col gap-lg p-md--rs">
                    <div className="flex w-full flex-row justify-between gap-md">
                        <LabelText
                            size={LabelTextSize.Medium}
                            label="Last Epoch Rewards"
                            text={typeof epochRewards === 'number' ? formattedEpochRewards : '0'}
                            showSupportingLabel
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
                            showSupportingLabel
                            tooltipText={
                                Number(rewardsPoolBalance) <= 0
                                    ? 'Coming soon'
                                    : 'The current balance in this validator’s reward pool.'
                            }
                            tooltipPosition={TooltipPosition.Right}
                        />
                    </div>
                </div>
            </Panel>
            <Panel>
                <Title title="Network Participation" />
                <div className="flex flex-col gap-lg p-md--rs">
                    <div className="flex w-full flex-row justify-between gap-md">
                        <LabelText
                            size={LabelTextSize.Medium}
                            label="Checkpoint Participation"
                            text="--"
                            showSupportingLabel={false}
                            tooltipText={
                                'Coming soon' ??
                                'The proportion of checkpoints that this validator has certified to date.'
                            }
                            tooltipPosition={TooltipPosition.Right}
                        />
                        <LabelText
                            size={LabelTextSize.Medium}
                            label="Voted Last Round"
                            text="--"
                            showSupportingLabel={false}
                            tooltipText={
                                'Coming soon' ??
                                'This validator’s participation in the voting for the most recent round.'
                            }
                            tooltipPosition={TooltipPosition.Right}
                        />
                    </div>
                    <div className="flex w-full flex-row justify-between gap-md">
                        <LabelText
                            size={LabelTextSize.Medium}
                            label="Tallying Score"
                            text={tallyingScore ?? '--'}
                            showSupportingLabel={false}
                            tooltipText={
                                !tallyingScore
                                    ? 'Coming soon'
                                    : 'A score created by validators to assess each other’s performance during Iota’s standard operations.'
                            }
                            tooltipPosition={TooltipPosition.Right}
                        />
                        <LabelText
                            size={LabelTextSize.Medium}
                            label="Last Narwhal round"
                            text="--"
                            showSupportingLabel={false}
                            tooltipText={
                                'Coming soon' ?? 'The most recent Narwhal round for this epoch.'
                            }
                            tooltipPosition={TooltipPosition.Right}
                        />
                    </div>
                    <div className="flex w-full flex-row justify-between gap-md">
                        <LabelText
                            size={LabelTextSize.Medium}
                            label="Proposed next epoch gas price"
                            text={nextEpochGasPriceAmount}
                            showSupportingLabel
                            supportingLabel="nano"
                            tooltipText="The gas price estimate provided by this validator for the upcoming epoch."
                            tooltipPosition={TooltipPosition.Right}
                        />
                    </div>
                </div>
            </Panel>
        </div>
    );
}
