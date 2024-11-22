// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import {
    useGetTimeBeforeEpochNumber,
    useTimeAgo,
    TimeUnit,
    NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_STARTS,
    NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_REDEEMABLE,
} from '../../index';

export function useStakeTxnInfo(startEpoch?: string | number) {
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

    return {
        stakedRewardsStartEpoch,
        timeBeforeStakeRewardsRedeemableAgoDisplay,
    };
}
