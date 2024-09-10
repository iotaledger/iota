// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaValidatorSummary } from '@iota/iota-sdk/client';
import { Heading } from '@iota/ui';

import { Card, Stats } from '~/components/ui';
import { DelegationAmount } from './DelegationAmount';

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

    return (
        <div className="flex flex-col items-stretch gap-5 md:flex-row">
            <div className="flex-grow">
                <Card spacing="lg" height="full">
                    <div className="flex basis-full flex-col gap-8 md:basis-1/3">
                        <Heading as="div" variant="heading4/semibold" color="steel-darker">
                            IOTA Staked on Validator
                        </Heading>
                        <div className="flex flex-col gap-8 lg:flex-row">
                            <Stats
                                label="Staking APY"
                                tooltip="This is the Annualized Percentage Yield of the a specific validator’s past operations. Note there is no guarantee this APY will be true in the future."
                                unavailable={apy === null}
                            >
                                {apy}%
                            </Stats>
                            <Stats
                                label="Total IOTA Staked"
                                tooltip="The total IOTA staked on the network by validators and delegators to validate the network and earn rewards."
                                unavailable={totalStake <= 0}
                            >
                                <DelegationAmount amount={totalStake} isStats />
                            </Stats>
                        </div>
                        <div className="flex flex-col gap-8 lg:flex-row">
                            <Stats
                                label="Commission"
                                tooltip="Fee charged by the validator for staking services"
                            >
                                <Heading as="h3" variant="heading2/semibold" color="steel-darker">
                                    {commission}%
                                </Heading>
                            </Stats>
                            <Stats
                                label="Delegators"
                                tooltip="The number of active delegators"
                                unavailable
                            />
                        </div>
                    </div>
                </Card>
            </div>

            <div className="flex-grow">
                <Card spacing="lg" height="full">
                    <div className="flex basis-full flex-col items-stretch gap-8 md:basis-80">
                        <Heading as="div" variant="heading4/semibold" color="steel-darker">
                            Validator Staking Rewards
                        </Heading>
                        <div className="flex flex-col gap-8">
                            <Stats
                                label="Last Epoch Rewards"
                                tooltip="The stake rewards collected during the last epoch."
                                unavailable={epochRewards === null}
                            >
                                <DelegationAmount
                                    amount={typeof epochRewards === 'number' ? epochRewards : 0n}
                                    isStats
                                />
                            </Stats>

                            <Stats
                                label="Reward Pool"
                                tooltip="Amount currently in this validator’s reward pool"
                                unavailable={Number(rewardsPoolBalance) <= 0}
                            >
                                <DelegationAmount amount={rewardsPoolBalance} isStats />
                            </Stats>
                        </div>
                    </div>
                </Card>
            </div>

            <div className="flex-grow">
                <Card spacing="lg" height="full">
                    <div className="flex max-w-full flex-col gap-8">
                        <Heading as="div" variant="heading4/semibold" color="steel-darker">
                            Network Participation
                        </Heading>
                        <div className="flex flex-col gap-8">
                            <div className="flex flex-col gap-8 lg:flex-row">
                                <Stats
                                    label="Checkpoint Participation"
                                    tooltip="The percentage of checkpoints certified thus far by this validator."
                                    unavailable
                                />

                                <Stats
                                    label="Voted Last Round"
                                    tooltip="Did this validator vote in the latest round."
                                    unavailable
                                />
                            </div>
                            <div className="flex flex-col gap-8 lg:flex-row">
                                <Stats
                                    label="Tallying Score"
                                    tooltip="A score generated by validators to evaluate each other’s performance throughout Iota’s regular operations."
                                    unavailable={!tallyingScore}
                                >
                                    {tallyingScore}
                                </Stats>
                                <Stats
                                    label="Last Narwhal Round"
                                    tooltip="Latest Narwhal round for this epoch."
                                    unavailable
                                />
                                <Stats
                                    label="Proposed Next Epoch Gas Price"
                                    tooltip="This validator's gas price quote for the next epoch."
                                >
                                    <DelegationAmount
                                        amount={validatorData.nextEpochGasPrice}
                                        isStats
                                        inNano
                                    />
                                </Stats>
                            </div>
                        </div>
                    </div>
                </Card>
            </div>
        </div>
    );
}
