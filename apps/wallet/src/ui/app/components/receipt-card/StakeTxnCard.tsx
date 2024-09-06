// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ValidatorLogo } from '_app/staking/validators/ValidatorLogo';
import { TxnAmount } from '_components';
import {
    NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_REDEEMABLE,
    NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_STARTS,
} from '_src/shared/constants';

import {
    formatPercentageDisplay,
    TimeUnit,
    useGetTimeBeforeEpochNumber,
    useGetValidatorsApy,
    useTimeAgo,
} from '@iota/core';
import type { IotaEvent } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

import { CardType, KeyValueInfo, Panel, TooltipPosition } from '@iota/apps-ui-kit';

interface StakeTxnCardProps {
    event: IotaEvent;
}

// For Staked Transaction use moveEvent Field to get the validator address, delegation amount, epoch
export function StakeTxnCard({ event }: StakeTxnCardProps) {
    const json = event.parsedJson as {
        amount: string;
        validator_address: string;
        epoch: string;
    };
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

    const timeBeforeStakeRewardsStartsAgo = useTimeAgo({
        timeFrom: timeBeforeStakeRewardsStarts,
        shortedTimeLabel: false,
        shouldEnd: true,
        maxTimeUnit: TimeUnit.ONE_HOUR,
    });
    const stakedRewardsStartEpoch =
        timeBeforeStakeRewardsStarts > 0
            ? `${timeBeforeStakeRewardsStartsAgo === '--' ? '' : 'in'} ${timeBeforeStakeRewardsStartsAgo}`
            : stakedEpoch
              ? `Epoch #${Number(startEarningRewardsEpoch)}`
              : '--';

    const { data: timeBeforeStakeRewardsRedeemable } =
        useGetTimeBeforeEpochNumber(redeemableRewardsEpoch);

    const timeBeforeStakeRewardsRedeemableAgo = useTimeAgo({
        timeFrom: timeBeforeStakeRewardsRedeemable,
        shortedTimeLabel: false,
        shouldEnd: true,
        maxTimeUnit: TimeUnit.ONE_HOUR,
    });
    const timeBeforeStakeRewardsRedeemableAgoDisplay =
        timeBeforeStakeRewardsRedeemable > 0
            ? `${timeBeforeStakeRewardsRedeemableAgo === '--' ? '' : 'in'} ${timeBeforeStakeRewardsRedeemableAgo}`
            : stakedEpoch
              ? `Epoch #${Number(redeemableRewardsEpoch)}`
              : '--';
    return (
        <div className="flex flex-col gap-2">
            {validatorAddress && (
                <ValidatorLogo
                    validatorAddress={validatorAddress}
                    type={CardType.Filled}
                    showActiveStatus
                    activeEpoch={json?.epoch}
                />
            )}
            {stakedAmount && (
                <TxnAmount amount={stakedAmount} coinType={IOTA_TYPE_ARG} subtitle="Stake" />
            )}
            <Panel hasBorder>
                <div className="flex flex-col gap-y-sm p-md">
                    <KeyValueInfo
                        keyText="APY"
                        valueText={formatPercentageDisplay(apy, '--', isApyApproxZero)}
                        tooltipText="This is the Annualized Percentage Yield of the a specific validator’s past operations. Note there is no guarantee this APY will be true in the future."
                        tooltipPosition={TooltipPosition.Right}
                    />
                    <KeyValueInfo
                        keyText="Staking Rewards Start"
                        valueText={stakedRewardsStartEpoch}
                    />
                    <KeyValueInfo
                        keyText="Redeem Rewards"
                        valueText={timeBeforeStakeRewardsRedeemableAgoDisplay}
                    />
                </div>
            </Panel>
        </div>
    );
}
