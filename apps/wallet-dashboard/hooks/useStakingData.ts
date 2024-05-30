// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from 'react';
import { useGetDelegatedStake } from '@mysten/core';
import { useCurrentAccount } from '@mysten/dapp-kit';
import { Stake } from '@/lib/types';

// Unit used to convert stake and rewards from raw values. This is temporary and will be removed once we have the correct conversion factor.
const HARDCODED_CONVERSION_UNIT = 1000000000;

export function useStakingData() {
    const account = useCurrentAccount();
    const { data: stakeData } = useGetDelegatedStake({
        address: account?.address || '',
    });

    const [totalStake, setTotalStake] = useState(0);
    const [totalRewards, setTotalRewards] = useState(0);
    const [stakingData, setStakingData] = useState<Stake[]>([]);

    useEffect(() => {
        if (!stakeData) return;

        let stakeSum = 0;
        let rewardSum = 0;
        const formattedStakes: Stake[] = [];

        stakeData.forEach((validator) => {
            validator.stakes.forEach((stake) => {
                stakeSum += Number(stake.principal);
                if (stake.status === 'Active') {
                    rewardSum += Number(stake.estimatedReward);
                }
                formattedStakes.push({
                    id: `${validator.validatorAddress}-${stake.stakeActiveEpoch}`,
                    validator: validator.validatorAddress,
                    stake: `${Number(stake.principal) / HARDCODED_CONVERSION_UNIT}`,
                    rewards:
                        stake.status === 'Active'
                            ? `${Number(stake.estimatedReward) / HARDCODED_CONVERSION_UNIT}`
                            : undefined,
                    stakeActiveEpoch: stake.stakeActiveEpoch,
                    stakeRequestEpoch: stake.stakeRequestEpoch,
                    status: stake.status,
                });
            });
        });

        setTotalStake(stakeSum);
        setTotalRewards(rewardSum);
        setStakingData(formattedStakes);
    }, [stakeData]);

    const totalStakeConverted = totalStake / HARDCODED_CONVERSION_UNIT;
    const totalRewardsConverted = totalRewards / HARDCODED_CONVERSION_UNIT;

    return {
        totalStakeConverted,
        totalRewardsConverted,
        stakingData,
    };
}
