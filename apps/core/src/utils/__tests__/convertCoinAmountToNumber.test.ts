// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, it, expect } from 'vitest';
import { convertCoinAmountToNumber } from '../convertCoinAmountToNumber';

describe('convertCoinAmountToNumber', () => {
    const IOTA_DECIMALS = 9;

    describe('bigint inputs', () => {
        it('should convert bigint amount correctly', () => {
            const amount = 123450000000n; // 123.45 IOTA
            expect(convertCoinAmountToNumber(amount, IOTA_DECIMALS)).toBe(123.45);
        });

        it('should handle zero bigint', () => {
            expect(convertCoinAmountToNumber(0n, IOTA_DECIMALS)).toBe(0);
        });

        it('should handle very large bigint amounts', () => {
            const amount = 1000000000000000000n; // 1,000,000,000 IOTA
            expect(convertCoinAmountToNumber(amount, IOTA_DECIMALS)).toBe(1000000000);
        });

        it('should handle small bigint amounts', () => {
            const amount = 1n; // 0.000000001 IOTA
            expect(convertCoinAmountToNumber(amount, IOTA_DECIMALS)).toBe(0.000000001);
        });

        it('should handle negative amounts', () => {
            const amount = -123450000000n;
            expect(convertCoinAmountToNumber(amount, IOTA_DECIMALS)).toBe(-123.45);
        });
    });

    describe('string inputs', () => {
        it('should convert string amount correctly', () => {
            const amount = '123450000000'; // 123.45 IOTA
            expect(convertCoinAmountToNumber(amount, IOTA_DECIMALS)).toBe(123.45);
        });

        it('should handle string zero', () => {
            expect(convertCoinAmountToNumber('0', IOTA_DECIMALS)).toBe(0);
        });

        it('should handle empty string', () => {
            expect(convertCoinAmountToNumber('', IOTA_DECIMALS)).toBe(0);
        });

        it('should handle very large string amounts', () => {
            const amount = '1000000000000000000';
            expect(convertCoinAmountToNumber(amount, IOTA_DECIMALS)).toBe(1000000000);
        });
    });

    describe('number inputs', () => {
        it('should convert number amount correctly', () => {
            const amount = 123450000000; // 123.45 IOTA
            expect(convertCoinAmountToNumber(amount, IOTA_DECIMALS)).toBe(123.45);
        });

        it('should handle zero number', () => {
            expect(convertCoinAmountToNumber(0, IOTA_DECIMALS)).toBe(0);
        });
    });

    describe('null and undefined inputs', () => {
        it('should return 0 for undefined', () => {
            expect(convertCoinAmountToNumber(undefined, IOTA_DECIMALS)).toBe(0);
        });

        it('should return 0 for null', () => {
            expect(convertCoinAmountToNumber(null, IOTA_DECIMALS)).toBe(0);
        });
    });
});
