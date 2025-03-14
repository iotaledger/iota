// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useRecognizedPackages } from '@iota/core';
import { useCoinsStore } from '_app/zustand/coins';
import { get } from 'idb-keyval';
import { useCallback, useEffect } from 'react';

const PINNED_COIN_TYPES = 'pinned-coin-types';

export function usePinnedCoinTypes() {
    const coinsStore = useCoinsStore();
    const setPinnedCoinTypes = coinsStore.setPinnedCoinTypes;
    const internalPinnedCoinTypes = coinsStore.pinnedCoinTypes;
    const recognizedPackages = useRecognizedPackages();

    useEffect(() => {
        (async () => {
            const pinnedCoins = await get<string[]>(PINNED_COIN_TYPES);

            if (pinnedCoins) {
                const filteredPinnedCoins = pinnedCoins.filter(
                    (coinType) => !recognizedPackages.includes(coinType.split('::')[0]),
                );
                setPinnedCoinTypes(PINNED_COIN_TYPES, filteredPinnedCoins);
            }
        })();
    }, [recognizedPackages, setPinnedCoinTypes]);

    const pinCoinType = useCallback(
        async (newCoinType: string) => {
            if (internalPinnedCoinTypes.find((coinType) => coinType === newCoinType)) return;

            const newPinnedCoinTypes = [...internalPinnedCoinTypes, newCoinType];
            setPinnedCoinTypes(PINNED_COIN_TYPES, newPinnedCoinTypes);
        },
        [internalPinnedCoinTypes, setPinnedCoinTypes],
    );

    const unpinCoinType = useCallback(
        async (removeCoinType: string) => {
            const newPinnedCoinTypes = internalPinnedCoinTypes.filter(
                (coinType) => coinType !== removeCoinType,
            );
            setPinnedCoinTypes(PINNED_COIN_TYPES, newPinnedCoinTypes);
        },
        [internalPinnedCoinTypes, setPinnedCoinTypes],
    );

    return [internalPinnedCoinTypes, { pinCoinType, unpinCoinType }] as const;
}
