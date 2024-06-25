// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type CoinBalance } from '@iota/iota.js/client';
import { getCoinSymbol, useCoinMetadata } from '../hooks';

export function filterAndSortTokenBalances(tokens: CoinBalance[]) {
    return tokens
        .filter((token) => Number(token.totalBalance) > 0)
        .sort((a, b) => {
            const aCoin = useCoinMetadata(a.coinType);
            const { data: aData } = aCoin;
            const aSymbol = aData?.symbol ?? getCoinSymbol(a.coinType);

            const bCoin = useCoinMetadata(b.coinType);
            const { data: bData } = bCoin;
            const bSymbol = bData?.symbol ?? getCoinSymbol(b.coinType);

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
