// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type DelegatedStake } from '@iota/iota-sdk/client';

// Get Stake IOTA by stakeIotaId
export function getStakeIotaByIotaId(allDelegation: DelegatedStake[], stakeIotaId?: string | null) {
    return (
        allDelegation.reduce((acc, curr) => {
            const total = BigInt(
                curr.stakes.find(({ stakedIotaId }) => stakedIotaId === stakeIotaId)?.principal ||
                    0,
            );
            return total + acc;
        }, 0n) || 0n
    );
}
