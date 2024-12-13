// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type DelegatedStake } from '@iota/iota-sdk/client';

// Get total Stake IOTA for a specific validator address
export function getTokenStakeIotaForValidator(
    allDelegation: DelegatedStake[],
    validatorAddress?: string | null,
) {
    return (
        allDelegation.reduce((acc, curr) => {
            if (validatorAddress === curr.validatorAddress) {
                return curr.stakes.reduce((total, { principal }) => total + BigInt(principal), acc);
            }
            return acc;
        }, 0n) || 0n
    );
}
