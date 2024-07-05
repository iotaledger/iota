// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type CoinBalance } from '@iota/iota.js/client';
import { getCoinSymbol } from '../hooks';
import { IOTA_TYPE_ARG } from '@iota/iota.js/utils';

/**
 * Filter and sort token balances by symbol and total balance.
 * IOTA tokens are always sorted first.
 * @param tokens The token balances to filter and sort.
 * @returns The filtered and sorted token balances.
 */
export function filterAndSortTokenBalances(tokens: CoinBalance[]) {
    return tokens
        .filter((token) => Number(token.totalBalance) > 0)
        .sort((a, b) => {
            if (a.coinType === IOTA_TYPE_ARG && b.coinType !== IOTA_TYPE_ARG) {
                return -1;
            } else if (a.coinType !== IOTA_TYPE_ARG && b.coinType === IOTA_TYPE_ARG) {
                return 1;
            }

            return (getCoinSymbol(a.coinType) + Number(a.totalBalance)).localeCompare(
                getCoinSymbol(a.coinType) + Number(b.totalBalance),
            );
        });
}
