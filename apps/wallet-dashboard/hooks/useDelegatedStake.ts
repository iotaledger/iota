// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Stake } from '@/lib/types';
import { useGetDelegatedStake } from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';

export function useDelegatedStake() {
    const account = useCurrentAccount();
    const { data: delegatedStakeData } = useGetDelegatedStake({
        address: account?.address || '',
    });

    let totalStake: bigint = BigInt(0);
    let totalRewards: bigint = BigInt(0);

    const delegatedStake = delegatedStakeData?.flatMap((delegatedStake) => {
        return delegatedStake.stakes.map((stake) => {
            totalStake += BigInt(stake.principal);
            if (stake.status === 'Active') {
                totalRewards += BigInt(stake.estimatedReward);
            }
            return {
                id: `${delegatedStake.validatorAddress}-${stake.stakeActiveEpoch}`,
                validator: delegatedStake.validatorAddress,
                stake: `${stake.principal}`,
                estimatedReward: stake.status === 'Active' ? `${stake.estimatedReward}` : undefined,
                stakeActiveEpoch: stake.stakeActiveEpoch,
                stakeRequestEpoch: stake.stakeRequestEpoch,
                status: stake.status,
                stakedIotaId: stake.stakedIotaId,
            } as Stake;
        });
    });

    return {
        totalStake,
        totalRewards,
        delegatedStake,
    };
}
