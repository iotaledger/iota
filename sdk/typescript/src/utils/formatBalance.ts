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
