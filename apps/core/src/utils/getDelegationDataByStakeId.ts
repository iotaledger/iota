// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { DelegatedStake, StakeObject } from '@iota/iota-sdk/client';

// Helper function to get the delegation by stakedIotaId
export function getDelegationDataByStakeId(
    delegationsStake: DelegatedStake[],
    stakeIotaId: string,
): StakeObject | null {
    for (const { stakes } of delegationsStake) {
        const stake = stakes.find(({ stakedIotaId }) => stakedIotaId === stakeIotaId) || null;
        if (stake) return stake;
    }

    return null;
}
