// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, it, expect } from 'vitest';
import { parseUnstakeAmountNanos, calculateUnstakeBreakdown } from '../calculateUnstakeAmounts';

describe('parseUnstakeAmountNanos', () => {
    it('should parse a valid amount string to nanos', () => {
        expect(parseUnstakeAmountNanos('1')).toBe(1_000_000_000n);
        expect(parseUnstakeAmountNanos('2.5')).toBe(2_500_000_000n);
        expect(parseUnstakeAmountNanos('0.1')).toBe(100_000_000n);
    });

    it('should return 0n for empty string', () => {
        expect(parseUnstakeAmountNanos('')).toBe(0n);
    });

    it('should return 0n for non-numeric input', () => {
        expect(parseUnstakeAmountNanos('abc')).toBe(0n);
    });

    it('should return 0n for zero', () => {
        expect(parseUnstakeAmountNanos('0')).toBe(0n);
    });

    it('should return 0n for negative values', () => {
        expect(parseUnstakeAmountNanos('-1')).toBe(0n);
    });
});

describe('calculateUnstakeBreakdown', () => {
    const principalAmount = 10_000_000_000n; // 10 IOTA
    const rewardAmount = 1_000_000_000n; // 1 IOTA reward

    it('should return full amounts for non-partial unstake', () => {
        const result = calculateUnstakeBreakdown({
            principalAmount,
            rewardAmount,
            unstakeAmountNanos: 0n,
            isPartialUnstake: false,
        });

        expect(result.unstakeAmount).toBe(principalAmount);
        expect(result.proportionalRewards).toBe(rewardAmount);
        expect(result.totalUnstakeAmount).toBe(principalAmount + rewardAmount);
        expect(result.remainingStake).toBe(0n);
        expect(result.remainingRewards).toBe(0n);
        expect(result.remainingTotalStaked).toBe(0n);
    });

    it('should calculate proportional amounts for partial unstake', () => {
        const unstakeAmountNanos = 5_000_000_000n; // 5 IOTA (half)
        const result = calculateUnstakeBreakdown({
            principalAmount,
            rewardAmount,
            unstakeAmountNanos,
            isPartialUnstake: true,
        });

        expect(result.unstakeAmount).toBe(5_000_000_000n);
        expect(result.proportionalRewards).toBe(500_000_000n); // half of rewards
        expect(result.totalUnstakeAmount).toBe(5_500_000_000n);
        expect(result.remainingStake).toBe(5_000_000_000n);
        expect(result.remainingRewards).toBe(500_000_000n);
        expect(result.remainingTotalStaked).toBe(5_500_000_000n);
    });

    it('should use full principal when partial unstake amount is 0', () => {
        const result = calculateUnstakeBreakdown({
            principalAmount,
            rewardAmount,
            unstakeAmountNanos: 0n,
            isPartialUnstake: true,
        });

        expect(result.unstakeAmount).toBe(principalAmount);
    });

    it('should handle zero reward amount', () => {
        const result = calculateUnstakeBreakdown({
            principalAmount,
            rewardAmount: 0n,
            unstakeAmountNanos: 5_000_000_000n,
            isPartialUnstake: true,
        });

        expect(result.proportionalRewards).toBe(0n);
        expect(result.remainingRewards).toBe(0n);
    });

    it('should handle zero principal amount', () => {
        const result = calculateUnstakeBreakdown({
            principalAmount: 0n,
            rewardAmount: 0n,
            unstakeAmountNanos: 0n,
            isPartialUnstake: false,
        });

        expect(result.unstakeAmount).toBe(0n);
        expect(result.proportionalRewards).toBe(0n);
    });
});
