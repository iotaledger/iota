// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import BigNumber from 'bignumber.js';

function formatAmountParts(amount?: BigNumber | bigint | number | string | null): string[] {
    if (typeof amount === 'undefined' || amount === null) {
        return ['--'];
    }

    let postfix = '';
    let bn = new BigNumber(amount.toString());
    const bnAbs = bn.abs();

    // use absolute value to determine the postfix
    if (bnAbs.gte(1_000_000_000)) {
        bn = bn.shiftedBy(-9);
        postfix = 'B';
    } else if (bnAbs.gte(1_000_000)) {
        bn = bn.shiftedBy(-6);
        postfix = 'M';
    } else if (bnAbs.gte(10_000)) {
        bn = bn.shiftedBy(-3);
        postfix = 'K';
    }

    if (bnAbs.gte(1)) {
        bn = bn.decimalPlaces(2, BigNumber.ROUND_DOWN);
    }

    if (bnAbs.gt(0) && bnAbs.lt(1)) {
        const leadingZeros = countDecimalLeadingZeros(bn.toFormat());

        if (leadingZeros >= 4) {
            return [formatWithSubscript(bn.toFormat(), leadingZeros), postfix];
        } else {
            return [bn.toFormat(leadingZeros + 1), postfix];
        }
    }

    return [bn.toFormat(), postfix];
}

export function formatAmount(...args: Parameters<typeof formatAmountParts>) {
    return formatAmountParts(...args)
        .filter(Boolean)
        .join(' ');
}

function countDecimalLeadingZeros(input: BigNumber | bigint | number | string | null): number {
    if (input === null) {
        return 0;
    }

    const [, decimals] = input.toString().split('.');

    if (!decimals) {
        return 0;
    }

    let count = 0;

    for (const digit of decimals) {
        if (digit === '0') {
            count++;
        } else {
            break;
        }
    }

    return count;
}

const SUBSCRIPTS = ['₀', '₁', '₂', '₃', '₄', '₅', '₆', '₇', '₈', '₉'];

export function formatWithSubscript(
    input: BigNumber | bigint | number | string | null,
    zeroCount: number,
): string {
    if (input === null) {
        return '0';
    }

    const [, decimals] = input.toString().split('.');
    const remainder = decimals.slice(zeroCount);

    const digits = zeroCount.toString().split('');
    const subscripts = digits.map((n) => SUBSCRIPTS[Number(n)]).join('');

    return `0.0${subscripts}${remainder}`;
}

export enum CoinFormat {
    Rounded = 'Rounded',
    Full = 'Full',
}

/**
 * Formats a coin balance based on our standard coin display logic.
 * If the balance is less than 1, it will be displayed in its full decimal form.
 * For values greater than 1, it will be truncated to 3 decimal places.
 */
export function formatBalance(
    balance: bigint | number | string,
    decimals: number,
    format: CoinFormat = CoinFormat.Rounded,
    showSign = false,
) {
    const bn = new BigNumber(balance.toString()).shiftedBy(-1 * decimals);
    let formattedBalance = formatAmount(bn);

    if (format === CoinFormat.Full) {
        formattedBalance = bn.toFormat();
    }

    if (showSign && !formattedBalance.startsWith('-')) {
        formattedBalance = `+${formattedBalance}`;
    }

    return formattedBalance;
}

/**
 * Converts a coin amount from base units to a number.
 * This is the inverse of parseAmount and is primarily used for analytics/data processing.
 * Avoids parsing formatted strings (e.g., "1,234.56" or "1.2 K").
 *
 * @param amount - The coin amount in base units
 * @param decimals - Coin decimals (e.g., 9 for IOTA)
 * @returns Numeric value, or 0 if invalid
 *
 * @example
 * ```typescript
 * formatBalanceToNumber(123450000000n, 9) // 123.45
 * formatBalanceToNumber("123450000000", 9) // 123.45
 * formatBalanceToNumber(undefined, 9) // 0
 * ```
 */
export function formatBalanceToNumber(
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
