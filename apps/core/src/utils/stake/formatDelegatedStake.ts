// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DelegatedStake, StakeObject } from '@iota/iota-sdk/client';

export type ExtendedDelegatedStake = StakeObject & {
    validatorAddress: string;
    estimatedReward?: string;
};

export function formatDelegatedStake(
    delegatedStakeData: DelegatedStake[],
): ExtendedDelegatedStake[] {
    return delegatedStakeData.flatMap((delegatedStake) => {
        return delegatedStake.stakes.map((stake) => {
            return {
                validatorAddress: delegatedStake.validatorAddress,
                estimatedReward: stake.status === 'Active' ? stake.estimatedReward : '',
                stakeActiveEpoch: stake.stakeActiveEpoch,
                stakeRequestEpoch: stake.stakeRequestEpoch,
                status: stake.status,
                stakedIotaId: stake.stakedIotaId,
                principal: stake.principal,
            };
        });
    });
}
