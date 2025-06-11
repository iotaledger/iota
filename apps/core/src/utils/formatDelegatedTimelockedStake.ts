// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DelegatedTimelockedStakeSchema } from './stake/types';
import type { DelegatedTimelockedStake } from '@iota/iota-sdk/client';
import type { ExtendedDelegatedTimelockedStake } from '../interfaces';

export function formatDelegatedTimelockedStake(
    delegatedTimelockedStakeData: DelegatedTimelockedStake[],
): ExtendedDelegatedTimelockedStake[] {
    return delegatedTimelockedStakeData.flatMap((delegatedTimelockedStake) => {
        const validatedDelegatedTimelockedStake =
            DelegatedTimelockedStakeSchema.parse(delegatedTimelockedStake);

        return validatedDelegatedTimelockedStake.stakes.map((stake) => {
            return {
                validatorAddress: delegatedTimelockedStake.validatorAddress,
                stakingPool: delegatedTimelockedStake.stakingPool,
                estimatedReward: stake.status === 'Active' ? stake.estimatedReward : '',
                stakeActiveEpoch: stake.stakeActiveEpoch,
                stakeRequestEpoch: stake.stakeRequestEpoch,
                status: stake.status,
                timelockedStakedIotaId: stake.timelockedStakedIotaId,
                principal: stake.principal,
                expirationTimestampMs: stake.expirationTimestampMs,
                label: stake.label,
            };
        });
    });
}
