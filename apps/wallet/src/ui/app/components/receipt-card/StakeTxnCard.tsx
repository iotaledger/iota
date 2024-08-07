// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ValidatorLogo } from '_app/staking/validators/ValidatorLogo';
import { TxnAmount } from '_components/receipt-card/TxnAmount';
import {
    NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_REDEEMABLE,
    NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_STARTS,
} from '_src/shared/constants';
import { CountDownTimer } from '_src/ui/app/shared/countdown-timer';
import { Text } from '_src/ui/app/shared/text';
import { IconTooltip } from '_src/ui/app/shared/tooltip';
import {
    formatPercentageDisplay,
    useGetTimeBeforeEpochNumber,
    useGetValidatorsApy,
} from '@iota/core';
import type { IotaEvent } from '@iota/iota.js/client';
import { IOTA_TYPE_ARG } from '@iota/iota.js/utils';

import { Card } from '../../shared/transaction-summary/Card';

interface StakeTxnCardProps {
    event: IotaEvent;
}

// For Staked Transaction use moveEvent Field to get the validator address, delegation amount, epoch
export function StakeTxnCard({ event }: StakeTxnCardProps) {
    const json = event.parsedJson as { amount: string; validator_address: string; epoch: string };
    const validatorAddress = json?.validator_address;
    const stakedAmount = json?.amount;
    const stakedEpoch = Number(json?.epoch || '0');

    const { data: rollingAverageApys } = useGetValidatorsApy();

    const { apy, isApyApproxZero } = rollingAverageApys?.[validatorAddress] ?? {
        apy: null,
    };
    // Reward will be available after 2 epochs
    // TODO: Get epochStartTimestampMs/StartDate
    // for staking epoch + NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_REDEEMABLE
    const startEarningRewardsEpoch =
        Number(stakedEpoch) + NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_STARTS;

    const redeemableRewardsEpoch =
        Number(stakedEpoch) + NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_REDEEMABLE;

    const { data: timeBeforeStakeRewardsStarts } =
        useGetTimeBeforeEpochNumber(startEarningRewardsEpoch);

    const { data: timeBeforeStakeRewardsRedeemable } =
        useGetTimeBeforeEpochNumber(redeemableRewardsEpoch);

    return (
        <Card>
            <div className="divide-gray-40 flex flex-col divide-x-0 divide-y divide-solid">
                {validatorAddress && (
                    <div className="divide-gray-40 mb-3.5 w-full divide-y divide-solid">
                        <ValidatorLogo
                            validatorAddress={validatorAddress}
                            showAddress
                            iconSize="md"
                            size="body"
                            activeEpoch={json?.epoch}
                        />
                    </div>
                )}
                {stakedAmount && (
                    <TxnAmount amount={stakedAmount} coinType={IOTA_TYPE_ARG} label="Stake" />
                )}
                <div className="flex flex-col">
                    <div className="flex w-full justify-between py-3.5">
                        <div className="text-steel flex items-baseline justify-center gap-1">
                            <Text variant="body" weight="medium" color="steel-darker">
                                APY
                            </Text>
                            <IconTooltip tip="This is the Annualized Percentage Yield of the a specific validator’s past operations. Note there is no guarantee this APY will be true in the future." />
                        </div>
                        <Text variant="body" weight="medium" color="steel-darker">
                            {formatPercentageDisplay(apy, '--', isApyApproxZero)}
                        </Text>
                    </div>
                </div>
                <div className="flex flex-col">
                    <div className="flex w-full justify-between py-3.5">
                        <div className="text-steel flex items-baseline gap-1">
                            <Text variant="body" weight="medium" color="steel-darker">
                                {timeBeforeStakeRewardsStarts > 0
                                    ? 'Staking Rewards Start'
                                    : 'Staking Rewards Started'}
                            </Text>
                        </div>

                        {timeBeforeStakeRewardsStarts > 0 ? (
                            <CountDownTimer
                                timestamp={timeBeforeStakeRewardsStarts}
                                variant="body"
                                color="steel-darker"
                                weight="medium"
                                label="in"
                                endLabel="--"
                            />
                        ) : (
                            <Text variant="body" weight="medium" color="steel-darker">
                                Epoch #{startEarningRewardsEpoch}
                            </Text>
                        )}
                    </div>
                    <div className="flex w-full justify-between">
                        <div className="text-steel flex flex-1 items-baseline gap-1">
                            <Text variant="pBody" weight="medium" color="steel-darker">
                                Staking Rewards Redeemable
                            </Text>
                        </div>
                        <div className="flex flex-1 items-center justify-end gap-1">
                            {timeBeforeStakeRewardsRedeemable > 0 ? (
                                <CountDownTimer
                                    timestamp={timeBeforeStakeRewardsRedeemable}
                                    variant="body"
                                    color="steel-darker"
                                    weight="medium"
                                    label="in"
                                    endLabel="--"
                                />
                            ) : (
                                <Text variant="body" weight="medium" color="steel-darker">
                                    Epoch #{redeemableRewardsEpoch}
                                </Text>
                            )}
                        </div>
                    </div>
                </div>
            </div>
        </Card>
    );
}
