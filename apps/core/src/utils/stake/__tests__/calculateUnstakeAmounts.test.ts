// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, it, expect } from 'vitest';
import { parseUnstakeAmountNanos, calculateUnstakeBreakdown } from '../calculateUnstakeAmounts';

describe('parseUnstakeAmountNanos', () => {
    it('should parse valid amount strings to nanos', () => {
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

    it('should handle very large numbers without precision loss', () => {
        // 1 billion IOTA
        expect(parseUnstakeAmountNanos('1000000000')).toBe(1_000_000_000_000_000_000n);
        // Large number with decimals
        expect(parseUnstakeAmountNanos('999999999.123456789')).toBe(999_999_999_123_456_789n);
    });

    it('should handle numbers with maximum precision (9 decimals)', () => {
        expect(parseUnstakeAmountNanos('1.000000001')).toBe(1_000_000_001n);
        expect(parseUnstakeAmountNanos('0.000000001')).toBe(1n);
    });

    it('should handle very small positive numbers', () => {
        expect(parseUnstakeAmountNanos('0.000000001')).toBe(1n);
        expect(parseUnstakeAmountNanos('0.000000002')).toBe(2n);
    });

    it('should handle edge cases that parseFloat would lose precision on', () => {
        // These numbers are known to have precision issues with parseFloat
        expect(parseUnstakeAmountNanos('0.1')).toBe(100_000_000n);
        expect(parseUnstakeAmountNanos('0.2')).toBe(200_000_000n);
        expect(parseUnstakeAmountNanos('0.3')).toBe(300_000_000n);
    });

    it('should handle strings with leading/trailing zeros', () => {
        expect(parseUnstakeAmountNanos('01.0')).toBe(1_000_000_000n);
        expect(parseUnstakeAmountNanos('1.00000000')).toBe(1_000_000_000n);
        expect(parseUnstakeAmountNanos('0.100000000')).toBe(100_000_000n);
    });

    it('should return 0n for Infinity', () => {
        expect(parseUnstakeAmountNanos('Infinity')).toBe(0n);
        expect(parseUnstakeAmountNanos('-Infinity')).toBe(0n);
    });

    it('should return 0n for special number strings', () => {
        expect(parseUnstakeAmountNanos('NaN')).toBe(0n);
        expect(parseUnstakeAmountNanos('null')).toBe(0n);
        expect(parseUnstakeAmountNanos('undefined')).toBe(0n);
    });

    it('should handle strings with whitespace', () => {
        expect(parseUnstakeAmountNanos(' 1.5 ')).toBe(1_500_000_000n);
        expect(parseUnstakeAmountNanos('  10  ')).toBe(10_000_000_000n);
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
