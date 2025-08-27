// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import BigNumber from 'bignumber.js';
import { IOTA_DECIMALS } from './constants.js';

export function parseIotaToNanos(amount: string) {
    try {
        return BigInt(new BigNumber(amount).shiftedBy(IOTA_DECIMALS).integerValue().toString());
    } catch (e) {
        return BigInt(0);
    }
}

export function safeParseIotaToNanos(amount: string) {
    try {
        return BigInt(new BigNumber(amount).shiftedBy(IOTA_DECIMALS).integerValue().toString());
    } catch (e) {
        return null;
    }
}
