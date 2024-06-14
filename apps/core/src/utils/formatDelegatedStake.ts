// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DelegatedStake, StakeObject } from '@iota/iota.js/client';

export type DelegatedStakeWithValidator = Extract<StakeObject, { estimatedReward?: string }> & {
    validatorAddress: string;
};

export function formatDelegatedStake(
    delegatedStakeData: DelegatedStake[],
): DelegatedStakeWithValidator[] {
    return (
        delegatedStakeData?.flatMap((delegatedStake) => {
            return delegatedStake.stakes.map((stake) => {
                return {
                    validatorAddress: delegatedStake.validatorAddress,
                    estimatedReward: stake.status === 'Active' ? stake.estimatedReward : undefined,
                    stakeActiveEpoch: stake.stakeActiveEpoch,
                    stakeRequestEpoch: stake.stakeRequestEpoch,
                    status: stake.status,
                    stakedIotaId: stake.stakedIotaId,
                    principal: stake.principal,
                };
            });
        }) || []
    );
}
