// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Stake } from '@/lib/types';
import { useGetDelegatedStake } from '@mysten/core';
import { useCurrentAccount } from '@mysten/dapp-kit';

// Unit used to convert stake and rewards from raw values. This is temporary and will be removed once we have the correct conversion factor.
const HARDCODED_CONVERSION_UNIT = 1000000000;

export function useStakingData() {
    const account = useCurrentAccount();
    const { data } = useGetDelegatedStake({
        address: account?.address || '',
    });
    const stakeData = data ?? [];

    let totalStake = 0;
    let totalRewards = 0;

    const stakingData = stakeData.flatMap((validator) => {
        return validator.stakes.map((stake) => {
            totalStake += Number(stake.principal);
            if (stake.status === 'Active') {
                totalRewards += Number(stake.estimatedReward);
            }
            return {
                id: `${validator.validatorAddress}-${stake.stakeActiveEpoch}`,
                validator: validator.validatorAddress,
                stake: `${Number(stake.principal) / HARDCODED_CONVERSION_UNIT}`,
                estimatedReward:
                    stake.status === 'Active'
                        ? `${Number(stake.estimatedReward) / HARDCODED_CONVERSION_UNIT}`
                        : undefined,
                stakeActiveEpoch: stake.stakeActiveEpoch,
                stakeRequestEpoch: stake.stakeRequestEpoch,
                status: stake.status,
            } as Stake;
        });
    });
    const totalStakeConverted = totalStake / HARDCODED_CONVERSION_UNIT;
    const totalRewardsConverted = totalRewards / HARDCODED_CONVERSION_UNIT;

    return {
        totalStakeConverted,
        totalRewardsConverted,
        stakingData,
    };
}
