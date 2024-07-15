// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useBackgroundClient } from './useBackgroundClient';
import { useQueryClient } from '@tanstack/react-query';
import { BIP44_COIN_TYPE, GAS_COIN_TYPE } from '../redux/slices/iota-objects/Coin';
import { type AllowedAccountTypes } from '_src/background/accounts-finder';

export interface UseAccountFinderOptions {
    accountType?: AllowedAccountTypes;
    bip44CoinType?: number;
    gasCoinType?: string;
    accountGapLimit?: number;
    addressGapLimit?: number;
    sourceID: string;
}

export function useAccountsFinder({
    bip44CoinType = BIP44_COIN_TYPE,
    gasCoinType = GAS_COIN_TYPE,
    addressGapLimit,
    accountGapLimit,
    sourceID,
    accountType,
}: UseAccountFinderOptions) {
    const backgroundClient = useBackgroundClient();
    const queryClient = useQueryClient();

    async function reset() {
        await backgroundClient.resetAccountsFinder();
        queryClient.invalidateQueries({
            queryKey: ['accounts-finder-results'],
        });
    }

    async function search() {
        if (!accountType) return;

        await backgroundClient.searchAccountsFinder({
            accountType,
            bip44CoinType: bip44CoinType,
            gasCoinType: gasCoinType,
            sourceID,
            accountGapLimit,
            addressGapLimit,
        });
        queryClient.invalidateQueries({
            queryKey: ['accounts-finder-results'],
        });
    }

    return {
        reset,
        search,
    };
}
