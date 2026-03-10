// Copyright (c) 2025 IOTA Stiftung

// SPDX-License-Identifier: Apache-2.0

import { describe, expect, test } from 'vitest';

import {
    COUPON_EXPIRED,
    INVALID_AVAILABLE_CLAIMS,
    INVALID_FOR_NAME_LENGTH,
    INVALID_PERCENTAGE,
    INVALID_USER,
    INVALID_YEARS,
    NON_STACKING_COUPON,
    validateCoupon,
    validateCoupons,
} from '../src/coupons';
import { Coupon } from '../src/types';

const DEFAULT_YEARS = 1;
const DEFAULT_LENGTH = 5;

function mockCoupon(config: {
    kind: 1 | 0;
    amount: string;
    available_claims?: string | null;
    user?: string | null;
    expiration?: string | null;
    length?: { from: number; to: number } | null;
    years?: { from: number; to: number } | null;
    can_stack?: boolean;
}): Coupon {
    return {
        couponCode: 'coup',
        kind: config.kind,
        amount: config.amount,
        rules: {
            available_claims: config.available_claims ?? '1',
            user: config.user ?? null,
            expiration: config.expiration ?? null,
            length: config.length ?? null,
            years: config.years ?? null,
            can_stack: config.can_stack ?? false,
        },
    };
}

describe('validateCoupon', () => {
    test('should pass if there are available claims', () => {
        const coupon = mockCoupon({
            kind: 1,
            amount: '1000',
            available_claims: '1',
        });
        expect(() => validateCoupon(coupon, DEFAULT_YEARS, DEFAULT_LENGTH)).not.toThrow();
    });

    test('should pass if available claims is undefined/null', () => {
        const coupon = mockCoupon({
            kind: 1,
            amount: '1000',
            available_claims: null,
        });
        expect(() => validateCoupon(coupon, DEFAULT_YEARS, DEFAULT_LENGTH)).not.toThrow();
    });

    test('should throw error if no available claims', () => {
        const coupon = mockCoupon({
            kind: 1,
            amount: '10000',
            available_claims: '0',
        });
        expect(() => validateCoupon(coupon, DEFAULT_YEARS, DEFAULT_LENGTH)).toThrow(
            INVALID_AVAILABLE_CLAIMS,
        );
    });

    test('should pass if years are within range', () => {
        const coupon = mockCoupon({
            kind: 1,
            amount: '1000',
            years: { from: 1, to: 3 },
        });

        expect(() => validateCoupon(coupon, 1, DEFAULT_LENGTH)).not.toThrow();
        expect(() => validateCoupon(coupon, 3, DEFAULT_LENGTH)).not.toThrow();
    });

    test('should pass if years rule is undefined/null', () => {
        const coupon = mockCoupon({
            kind: 1,
            amount: '1000',
            available_claims: '1',
            years: null,
        });
        expect(() => validateCoupon(coupon, DEFAULT_YEARS, DEFAULT_LENGTH)).not.toThrow();
    });

    test('should throw error when outside the year range (over)', () => {
        const coupon = mockCoupon({
            kind: 1,
            amount: '1000',
            available_claims: '1',
            years: { from: 1, to: 3 },
        });
        expect(() => validateCoupon(coupon, 4, DEFAULT_LENGTH)).toThrow(INVALID_YEARS);
    });

    test('should throw error when outside the year range (under)', () => {
        const coupon = mockCoupon({
            kind: 1,
            amount: '1000',
            available_claims: '1',
            years: { from: 2, to: 4 },
        });
        expect(() => validateCoupon(coupon, 1, DEFAULT_LENGTH)).toThrow(INVALID_YEARS);
    });

    test('should pass with percentage coupon', () => {
        const coupon = mockCoupon({
            kind: 0,
            amount: '50',
            available_claims: '1',
        });
        expect(() => validateCoupon(coupon, DEFAULT_YEARS, DEFAULT_LENGTH)).not.toThrow();
    });

    test('should throw an error with invalid percentage', () => {
        const coupon500 = mockCoupon({
            kind: 0,
            amount: '500',
            available_claims: '1',
        });
        const coupon0 = mockCoupon({
            kind: 0,
            amount: '0',
            available_claims: '1',
        });
        const couponNegative = mockCoupon({
            kind: 0,
            amount: '-50',
            available_claims: '1',
        });
        expect(() => validateCoupon(coupon500, DEFAULT_YEARS, DEFAULT_LENGTH)).toThrow(
            INVALID_PERCENTAGE,
        );
        expect(() => validateCoupon(coupon0, DEFAULT_YEARS, DEFAULT_LENGTH)).toThrow(
            INVALID_PERCENTAGE,
        );
        expect(() => validateCoupon(couponNegative, DEFAULT_YEARS, DEFAULT_LENGTH)).toThrow(
            INVALID_PERCENTAGE,
        );
    });

    test('should pass if length is within range', () => {
        const coupon = mockCoupon({
            kind: 1,
            amount: '1000',
            available_claims: '1',
            length: { from: 3, to: 5 },
        });
        expect(() => validateCoupon(coupon, DEFAULT_YEARS, 3)).not.toThrow();
        expect(() => validateCoupon(coupon, DEFAULT_YEARS, 4)).not.toThrow();
        expect(() => validateCoupon(coupon, DEFAULT_YEARS, 5)).not.toThrow();
    });

    test('should pass if length is not defined', () => {
        const coupon = mockCoupon({
            kind: 1,
            amount: '1000',
            available_claims: '1',
            length: null,
        });
        expect(() => validateCoupon(coupon, DEFAULT_YEARS, 3)).not.toThrow();
    });

    test('should throw error when length is outside rules range', () => {
        const coupon = mockCoupon({
            kind: 1,
            amount: '1000',
            available_claims: '1',
            length: { from: 4, to: 6 },
        });
        expect(() => validateCoupon(coupon, DEFAULT_YEARS, 3)).toThrow(INVALID_FOR_NAME_LENGTH);
        expect(() => validateCoupon(coupon, DEFAULT_YEARS, 7)).toThrow(INVALID_FOR_NAME_LENGTH);
    });

    test('should pass if user address is correct', () => {
        const coupon = mockCoupon({
            kind: 1,
            amount: '1000',
            available_claims: '1',
            user: '0xd17a0271bb4ae0a12af1a6d458805b6220603bb73a025d8da6d5c4cc848395e1',
        });
        expect(() =>
            validateCoupon(
                coupon,
                DEFAULT_YEARS,
                DEFAULT_LENGTH,

                '0xd17a0271bb4ae0a12af1a6d458805b6220603bb73a025d8da6d5c4cc848395e1',
            ),
        ).not.toThrow();
    });

    test('should pass if user address is undefined/null', () => {
        const coupon = mockCoupon({
            kind: 1,
            amount: '1000',
            available_claims: '1',
            user: null,
        });
        expect(() =>
            validateCoupon(
                coupon,
                DEFAULT_YEARS,
                DEFAULT_LENGTH,
                '0xd17a0271bb4ae0a12af1a6d458805b6220603bb73a025d8da6d5c4cc848395e1',
            ),
        ).not.toThrow();
    });

    test('should throw an error if user is not a match', () => {
        const coupon = mockCoupon({
            kind: 1,
            amount: '10000',
            available_claims: '1',
            user: '0xd17a0271bb4ae0a12af1a6d458805b6220603bb73a025d8da6d5c4cc848395e1',
        });
        expect(() =>
            validateCoupon(
                coupon,
                DEFAULT_YEARS,
                DEFAULT_LENGTH,
                '0x8a1bf46b9c1fee6864c7c2366f381101c369a91b1060ee6af92f49804a9792d5',
            ),
        ).toThrow(INVALID_USER);
    });

    test('should pass if the coupon is not expired', () => {
        const coupon = mockCoupon({
            kind: 1,
            amount: '1000',
            available_claims: '1',
            expiration: (Date.now() + 100000).toString(),
        });
        expect(() => validateCoupon(coupon, DEFAULT_YEARS, DEFAULT_LENGTH)).not.toThrow();
    });

    test('should pass if the coupon expiration is undefined/null', () => {
        const coupon = mockCoupon({
            kind: 1,
            amount: '1000',
            available_claims: '1',
            expiration: null,
        });
        expect(() => validateCoupon(coupon, DEFAULT_YEARS, DEFAULT_LENGTH)).not.toThrow();
    });

    test('should throw an error if the coupon is expired', () => {
        const coupon = mockCoupon({
            kind: 1,
            amount: '10000',
            available_claims: '1',
            expiration: (Date.now() - 100000).toString(),
        });
        expect(() => validateCoupon(coupon, DEFAULT_YEARS, DEFAULT_LENGTH)).toThrow(COUPON_EXPIRED);
    });
});

describe('validateCoupons (multi)', () => {
    test('should pass if all coupons are stackable', () => {
        const coupons = [
            mockCoupon({
                kind: 0,
                amount: '30',
                available_claims: '5',
                can_stack: true,
            }),
            mockCoupon({
                kind: 1,
                amount: '1000',
                available_claims: '1',
                can_stack: true,
            }),
            mockCoupon({
                kind: 1,
                amount: '500',
                available_claims: '1',
                can_stack: true,
            }),
        ];

        expect(() => validateCoupons(coupons, DEFAULT_YEARS, DEFAULT_LENGTH)).not.toThrow();
    });

    test('should fail if a coupon is NOT stackable', () => {
        const coupons = [
            mockCoupon({
                kind: 0,
                amount: '30',
                available_claims: '5',
                can_stack: true,
            }),
            mockCoupon({
                kind: 1,
                amount: '1000',
                available_claims: '1',
                can_stack: false, // <----
            }),
            mockCoupon({
                kind: 1,
                amount: '500',
                available_claims: '1',
                can_stack: true,
            }),
        ];

        expect(() => validateCoupons(coupons, DEFAULT_YEARS, DEFAULT_LENGTH)).toThrow(
            NON_STACKING_COUPON,
        );
    });
});
