// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Coupon } from './types';

export const INVALID_YEARS = 'Coupon is not valid for the given number of years.';
export const INVALID_FOR_NAME_LENGTH = 'Coupon is not valid for the given name length.';
export const INVALID_PERCENTAGE = 'Invalid percentage amount for coupon.';
export const INVALID_USER = 'Coupon address does not match.';
export const COUPON_EXPIRED = 'Coupon has expired.';
export const INVALID_AVAILABLE_CLAIMS = 'Number of claims cannot be zero.';
export const NON_STACKING_COUPON = 'Coupon cannot be used with other coupons.';

export function validateCoupon(coupon: Coupon, years: number, length: number, address?: string) {
    try {
        hasAvailableClaims(coupon.rules);
        isCouponValidForNameYears(coupon.rules, years);
        // Percentage off coupon
        if (coupon.kind === 0) {
            isValidCouponPercentage(coupon.amount);
        }
        isCouponValidForNameLength(coupon.rules, length);
        isCouponValidForAddress(coupon.rules, address);
        isCouponExpired(coupon.rules);
    } catch (error: unknown) {
        throw new Error(
            `Coupon '${coupon.couponCode}' validation failed: ${(error as Error)?.message}`,
        );
    }
}

export function validateCoupons(
    coupons: Coupon[],
    years: number,
    length: number,
    address?: string,
) {
    for (const coupon of coupons) {
        if (!coupon.rules.can_stack && coupons.length > 1) {
            throw new Error(
                `Coupon '${coupon.couponCode}' validation failed: ${NON_STACKING_COUPON}`,
            );
        }
        validateCoupon(coupon, years, length, address);
    }
}

export function applyCouponsToPrice(coupons: Coupon[], initialPrice: number): number {
    if (!coupons || coupons.length === 0) {
        return initialPrice;
    }

    let price = initialPrice;

    for (const coupon of coupons) {
        if (!coupon.rules.can_stack && coupons.length > 1) {
            throw new Error('Coupons provided cannot be stacked');
        }

        price = applyCouponToPrice(price, coupon);
    }

    return price;
}

export function applyCouponToPrice(price: number, coupon?: Coupon): number {
    if (!coupon) {
        return price;
    }

    const couponAmount = Number(coupon.amount);

    // 0 => percentage off
    // 1 => fixed amount off,
    if (coupon.kind === 0) {
        let discountAmount = (price * couponAmount) / 100;
        return price - discountAmount;
    } else if (coupon.kind === 1) {
        const discountedAmount = price - couponAmount;

        return discountedAmount < 0 ? 0 : discountedAmount;
    } else {
        throw new Error(`Unknown coupon kind: ${coupon.kind}`);
    }
}

export function hasAvailableClaims(rules: { available_claims?: string | null }) {
    if (
        rules?.available_claims !== null &&
        rules?.available_claims !== undefined &&
        parseInt(rules?.available_claims) <= 0
    ) {
        throw new Error(INVALID_AVAILABLE_CLAIMS);
    }
}

export function isCouponValidForNameYears(
    rules: { years?: { from: number; to: number } | null },
    years: number,
) {
    const { from: minYears, to: maxYears } = rules.years || {};
    if (minYears && maxYears && (years < minYears || years > maxYears)) {
        throw new Error(INVALID_YEARS);
    }
}

export function isValidCouponPercentage(amount: string) {
    if (parseInt(amount) <= 0 || parseInt(amount) > 100) {
        throw new Error(INVALID_PERCENTAGE);
    }
}

export function isCouponValidForNameLength(
    rules: { length?: { from: number; to: number } | null },
    length: number,
) {
    const { from: minLength, to: maxLength } = rules.length || {};
    if (minLength && maxLength && (length < minLength || length > maxLength)) {
        throw new Error(INVALID_FOR_NAME_LENGTH);
    }
}

export function isCouponValidForAddress(rules: { user?: string | null }, userAddress?: string) {
    if (rules.user && rules.user !== userAddress) {
        throw new Error(INVALID_USER);
    }
}

export function isCouponExpired(
    rules: { expiration?: string | null },
    currentTimestamp: string = Date.now().toString(),
) {
    if (rules.expiration && Number(currentTimestamp) > Number(rules.expiration)) {
        throw new Error(COUPON_EXPIRED);
    }
}
