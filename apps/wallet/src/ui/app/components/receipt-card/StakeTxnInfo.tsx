// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Divider, KeyValueInfo, Panel, TooltipPosition } from '@iota/apps-ui-kit';
import {
    NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_REDEEMABLE,
    NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_STARTS,
} from '_src/shared/constants';
import { useGetTimeBeforeEpochNumber, useTimeAgo, TimeUnit, type GasSummaryType } from '@iota/core';
import { GasSummary } from '../../shared/transaction-summary/cards/GasSummary';

interface StakeTxnInfoProps {
    apy?: string;
    startEpoch?: string | number;
    gasSummary?: GasSummaryType;
}

export function StakeTxnInfo({ apy, startEpoch, gasSummary }: StakeTxnInfoProps) {
    const startEarningRewardsEpoch =
        Number(startEpoch || 0) + NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_STARTS;

    const redeemableRewardsEpoch =
        Number(startEpoch || 0) + NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_REDEEMABLE;

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
            : startEpoch
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
            : startEpoch
              ? `Epoch #${Number(redeemableRewardsEpoch)}`
              : '--';
    return (
        <Panel hasBorder>
            <div className="flex flex-col gap-y-sm p-md">
                {apy && (
                    <KeyValueInfo
                        keyText="APY"
                        valueText={apy}
                        tooltipText="This is the Annualized Percentage Yield of the a specific validator’s past operations. Note there is no guarantee this APY will be true in the future."
                        tooltipPosition={TooltipPosition.Right}
                        fullwidth
                    />
                )}
                <KeyValueInfo
                    keyText="Staking Rewards Start"
                    valueText={stakedRewardsStartEpoch}
                    fullwidth
                />
                <KeyValueInfo
                    keyText="Redeem Rewards"
                    valueText={timeBeforeStakeRewardsRedeemableAgoDisplay}
                    fullwidth
                />
                <Divider />
                <GasSummary gasSummary={gasSummary} />
            </div>
        </Panel>
    );
}
