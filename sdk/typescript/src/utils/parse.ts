// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import BigNumber from 'bignumber.js';

export function parseIotaToNanos(amount: string, coinDecimals: number) {
    try {
        return BigInt(new BigNumber(amount).shiftedBy(coinDecimals).integerValue().toString());
    } catch (e) {
        return BigInt(0);
    }
}

export function safeParseIotaToNanos(amount: string, coinDecimals: number) {
    try {
        return BigInt(new BigNumber(amount).shiftedBy(coinDecimals).integerValue().toString());
    } catch (e) {
        return null;
    }
}
