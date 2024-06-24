// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type CoinBalance } from '@iota/iota.js/client';

// Sort tokens by symbol and total balance
// Move this to the API backend
// Filter out tokens with zero balance
export function filterAndSortTokenBalances(tokens: CoinBalance[]) {
    return tokens
        .filter((token) => Number(token.totalBalance) > 0)
        .sort((a, b) => {
            const aSymbol = getCoinSymbol(a.coinType);
            const bSymbol = getCoinSymbol(b.coinType);

            // Verify if any of the tokens is IOTA
            if (aSymbol === 'IOTA' && bSymbol !== 'IOTA') {
                return -1; // a is IOTA, should go first
            } else if (aSymbol !== 'IOTA' && bSymbol === 'IOTA') {
                return 1; // b is IOTA, should go first
            }

            // Sort by symbol and total balance
            return (aSymbol + Number(a.totalBalance)).localeCompare(
                bSymbol + Number(b.totalBalance),
            );
        });
}

export function getCoinSymbol(coinTypeArg: string) {
    return coinTypeArg.substring(coinTypeArg.lastIndexOf(':') + 1);
}
