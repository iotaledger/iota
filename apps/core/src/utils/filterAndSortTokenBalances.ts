// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type CoinBalance } from '@iota/iota.js/client';
import { getCoinSymbol } from '../hooks';
import { IOTA_TYPE_ARG } from '@iota/iota.js/utils';

export function filterAndSortTokenBalances(tokens: CoinBalance[]) {
    return tokens
        .filter((token) => Number(token.totalBalance) > 0)
        .sort((a, b) => {
            // Check if one of the tokens is IOTA
            if (a.coinType === IOTA_TYPE_ARG && b.coinType !== IOTA_TYPE_ARG) {
                return -1; // a is IOTA, should go first
            } else if (a.coinType !== IOTA_TYPE_ARG && b.coinType === IOTA_TYPE_ARG) {
                return 1; // b is IOTA, should go first
            }

            // Sort by symbol and then by balance
            return (getCoinSymbol(a.coinType) + Number(a.totalBalance)).localeCompare(
                getCoinSymbol(a.coinType) + Number(b.totalBalance),
            );
        });
}
