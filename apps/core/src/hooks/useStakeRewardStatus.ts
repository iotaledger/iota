// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_REDEEMABLE } from '../constants';
import { useFormatCoin, useGetTimeBeforeEpochNumber, useTimeAgo, TimeUnit } from '.';
import { determineCountDownText } from '../utils';

export function useStakeRewardStatus({
    stakeRequestEpoch,
    currentEpoch,
    inactiveValidator,
    activeButNotInTheCommittee,
    estimatedReward,
}: {
    stakeRequestEpoch: string;
    currentEpoch: number;
    inactiveValidator: boolean;
    activeButNotInTheCommittee?: boolean;
    estimatedReward?: string | number | bigint;
}) {
    // TODO: Once two step withdraw is available, add cool down and withdraw now logic
    // For cool down epoch, show Available to withdraw add rewards to principal
    // Reward earning epoch is 2 epochs after stake request epoch
    const earningRewardsEpoch =
        Number(stakeRequestEpoch) + NUM_OF_EPOCH_BEFORE_STAKING_REWARDS_REDEEMABLE;

    const isEarnedRewards = currentEpoch >= Number(earningRewardsEpoch);

    const delegationState =
        inactiveValidator || activeButNotInTheCommittee
            ? StakeState.NotEarning
            : isEarnedRewards
              ? StakeState.Earning
              : StakeState.WarmUp;

    const rewards = isEarnedRewards && estimatedReward ? BigInt(estimatedReward) : 0n;

    const [rewardsStaked, symbol] = useFormatCoin({ balance: rewards });

    // Applicable only for warm up
    const epochBeforeRewards = delegationState === StakeState.WarmUp ? earningRewardsEpoch : null;

    const statusText = {
        // Epoch time before earning
        [StakeState.WarmUp]: `Epoch #${earningRewardsEpoch}`,
        [StakeState.Earning]: `${rewardsStaked} ${symbol}`,
        // Epoch time before redrawing
        [StakeState.CoolDown]: `Epoch #`,
        [StakeState.Withdraw]: 'Now',
        [StakeState.NotEarning]: `${rewardsStaked} ${symbol}`,
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
export enum StakeState {
    WarmUp = 'WARM_UP',
    Earning = 'EARNING',
    CoolDown = 'COOL_DOWN',
    Withdraw = 'WITHDRAW',
    NotEarning = 'NOT_EARNING',
}
export const STATUS_COPY: {
    [key in StakeState]: string;
} = {
    [StakeState.WarmUp]: 'Starts Earning',
    [StakeState.Earning]: 'Staking Rewards',
    [StakeState.CoolDown]: 'Available to withdraw',
    [StakeState.Withdraw]: 'Withdraw',
    [StakeState.NotEarning]: 'Rewards',
};
