// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { NANOS_PER_IOTA } from '@iota/iota-sdk/utils';

/**
 * Parse a user-entered unstake amount string (in IOTA) to nanos (bigint).
 * Returns 0n for invalid or non-positive input.
 */
export function parseUnstakeAmountNanos(amountStr: string): bigint {
    const parsed = parseFloat(amountStr);
    if (!amountStr || isNaN(parsed) || parsed <= 0) {
        return 0n;
    }
    return BigInt(Math.floor(parsed * Number(NANOS_PER_IOTA)));
}

interface CalculateUnstakeBreakdownParams {
    principalAmount: bigint;
    rewardAmount: bigint;
    unstakeAmountNanos: bigint;
    isPartialUnstake: boolean;
}

export interface UnstakeBreakdown {
    unstakeAmount: bigint;
    proportionalRewards: bigint;
    totalUnstakeAmount: bigint;
    remainingStake: bigint;
    remainingRewards: bigint;
    remainingTotalStaked: bigint;
}

/**
 * Calculate the full breakdown of amounts for an unstake operation,
 * including proportional rewards and remaining balances.
 */
export function calculateUnstakeBreakdown({
    principalAmount,
    rewardAmount,
    unstakeAmountNanos,
    isPartialUnstake,
}: CalculateUnstakeBreakdownParams): UnstakeBreakdown {
    const unstakeAmount =
        isPartialUnstake && unstakeAmountNanos > 0n ? unstakeAmountNanos : principalAmount;

    const proportionalRewards =
        principalAmount > 0n ? (rewardAmount * unstakeAmount) / principalAmount : 0n;

    const totalUnstakeAmount = unstakeAmount + proportionalRewards;
    const remainingStake = principalAmount - unstakeAmount;
    const remainingRewards = rewardAmount - proportionalRewards;
    const remainingTotalStaked = remainingStake + remainingRewards;

    return {
        unstakeAmount,
        proportionalRewards,
        totalUnstakeAmount,
        remainingStake,
        remainingRewards,
        remainingTotalStaked,
    };
}
