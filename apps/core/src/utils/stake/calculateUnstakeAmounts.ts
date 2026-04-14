// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IOTA_DECIMALS, parseAmount } from '@iota/iota-sdk/utils';

/**
 * Parse a user-entered unstake amount string (in IOTA) to nanos (bigint).
 * Returns 0n for invalid or non-positive input.
 * Uses the SDK's parseAmount for precision-safe parsing.
 *
 * Note: The Input component limits decimal input to IOTA_DECIMALS (9 digits),
 * so no fractional nanos can occur, making parseAmount safe to use.
 */
export function parseUnstakeAmountNanos(amountStr: string): bigint {
    // Use SDK's parseAmount which handles BigNumber conversion safely
    const parsed = parseAmount(amountStr, IOTA_DECIMALS);

    // Return 0n for zero or negative values (unstake amount must be positive)
    return parsed > 0n ? parsed : 0n;
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

    // Use truncating division to match on-chain contract behavior
    // The contract uses exchange rates with truncating division (rounds down)
    // Frontend preview must match this to avoid showing incorrect amounts to users
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
