// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import BigNumber from 'bignumber.js';

/**
 * Converts a coin amount to a number (primarily for analytics purposes).
 * Avoids parsing formatted strings (e.g., "1,234.56" or "1.2 K").
 *
 * @param amount - The coin amount in base units
 * @param decimals - Coin decimals (e.g., 9 for IOTA)
 * @returns Numeric value for analytics, or 0 if invalid
 */
export function convertCoinAmountToNumber(
    amount: bigint | number | string | undefined | null,
    decimals: number,
): number {
    if (amount === undefined || amount === null || amount === '') {
        return 0;
    }

    try {
        const result = new BigNumber(amount.toString()).shiftedBy(-decimals).toNumber();
        return isNaN(result) ? 0 : result;
    } catch {
        return 0;
    }
}
