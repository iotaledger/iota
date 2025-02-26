// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type CoinBalance } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useMemo } from 'react';
import { useRecognizedPackages } from './useRecognizedPackages';

function sortCoins(balances: CoinBalance[]) {
    return balances.sort((a, b) => {
        if (a.coinType === IOTA_TYPE_ARG) {
            return -1;
        }

        return a.coinType.toLowerCase().localeCompare(b.coinType.toLowerCase());
    });
}

export function useSortedCoinsByCategories(
    coinBalances: CoinBalance[],
    pinnedCoinTypes?: string[],
) {
    const recognizedPackages = useRecognizedPackages();
    return useMemo(() => {
        const reducedCoinBalances = coinBalances?.reduce(
            (acc, coinBalance) => {
                if (recognizedPackages.includes(coinBalance.coinType.split('::')[0])) {
                    acc.recognized.push(coinBalance);
                } else if (pinnedCoinTypes?.includes(coinBalance.coinType)) {
                    acc.pinned.push(coinBalance);
                } else {
                    acc.unrecognized.push(coinBalance);
                }
                return acc;
            },
            {
                recognized: [] as CoinBalance[],
                pinned: [] as CoinBalance[],
                unrecognized: [] as CoinBalance[],
            },
        ) ?? { recognized: [], pinned: [], unrecognized: [] };

        return {
            recognized: sortCoins(reducedCoinBalances.recognized),
            pinned: sortCoins(reducedCoinBalances.pinned),
            unrecognized: sortCoins(reducedCoinBalances.unrecognized),
        };
    }, [coinBalances, recognizedPackages, pinnedCoinTypes]);
}
