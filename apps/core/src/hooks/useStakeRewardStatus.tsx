// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_REDEEMABLE } from '../constants';
import { useFormatCoin, useGetTimeBeforeEpochNumber, useTimeAgo, TimeUnit } from '.';
import { determineCountDownText } from '../utils';
import { StakeState, STATUS_COPY } from '../components/stake/StakedCard';

export function useStakeRewardStatus({
    stakeRequestEpoch,
    currentEpoch,
    inactiveValidator,
    estimatedReward,
}: {
    stakeRequestEpoch: string;
    currentEpoch: number;
    inactiveValidator: boolean;
    estimatedReward?: string | number;
}) {
    // TODO: Once two step withdraw is available, add cool down and withdraw now logic
    // For cool down epoch, show Available to withdraw add rewards to principal
    // Reward earning epoch is 2 epochs after stake request epoch
    const earningRewardsEpoch =
        Number(stakeRequestEpoch) + NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_REDEEMABLE;

    const isEarnedRewards = currentEpoch >= Number(earningRewardsEpoch);

    const delegationState = inactiveValidator
        ? StakeState.InActive
        : isEarnedRewards
          ? StakeState.Earning
          : StakeState.WarmUp;

    const rewards = isEarnedRewards && estimatedReward ? BigInt(estimatedReward) : 0n;

    const [rewardsStaked, symbol] = useFormatCoin(rewards, IOTA_TYPE_ARG);

    // Applicable only for warm up
    const epochBeforeRewards = delegationState === StakeState.WarmUp ? earningRewardsEpoch : null;

    const statusText = {
        // Epoch time before earning
        [StakeState.WarmUp]: `Epoch #${earningRewardsEpoch}`,
        [StakeState.Earning]: `${rewardsStaked} ${symbol}`,
        // Epoch time before redrawing
        [StakeState.CoolDown]: `Epoch #`,
        [StakeState.Withdraw]: 'Now',
        [StakeState.InActive]: 'Not earning rewards',
    };

    const { data: rewardEpochTime } = useGetTimeBeforeEpochNumber(Number(epochBeforeRewards) || 0);

    const timeAgo = useTimeAgo({
        timeFrom: rewardEpochTime || null,
        shortedTimeLabel: false,
        shouldEnd: true,
        maxTimeUnit: TimeUnit.ONE_HOUR,
    });

    const rewardTime = () => {
        if (Number(epochBeforeRewards) && rewardEpochTime > 0) {
            return determineCountDownText({
                timeAgo,
                label: 'in',
            });
        }

        return statusText[delegationState];
    };

    return {
        rewards,
        title: rewardTime(),
        subtitle: STATUS_COPY[delegationState],
    };
}
