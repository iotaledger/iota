// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinFormat, formatBalance, IOTA_DECIMALS } from '@iota/iota-sdk/utils';
import { BigNumber } from 'bignumber.js';

import { IOTA_TOKEN_SYMBOL } from '@/lib/constants';

interface FormatNanosToIotaOptions {
    formatRounded?: boolean;
    showSign?: boolean;
    showIotaSymbol?: boolean;
}

export function formatNanosToIota(
    balance: bigint | number | string,
    {
        formatRounded = true,
        showSign = false,
        showIotaSymbol = true,
    }: FormatNanosToIotaOptions = {},
) {
    const formattedBalance = formatBalance(
        balance,
        IOTA_DECIMALS,
        formatRounded ? CoinFormat.Rounded : CoinFormat.Full,
        showSign,
    );

    return showIotaSymbol ? `${formattedBalance} ${IOTA_TOKEN_SYMBOL}` : formattedBalance;
}

export function parseNanosToIota(balance: bigint | number | string): number {
    const parsed = new BigNumber(balance.toString()).shiftedBy(-1 * IOTA_DECIMALS);
    return Number(parsed);
}

export function parseIotaToNanos(balance: bigint | number | string): number {
    const parsed = new BigNumber(balance.toString()).shiftedBy(IOTA_DECIMALS);
    return Number(parsed);
}
