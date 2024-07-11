// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useBackgroundClient } from './useBackgroundClient';
import { useQueryClient } from '@tanstack/react-query';
import { IOTA_COIN_TYPE_ID, GAS_TYPE_ARG } from '../redux/slices/iota-objects/Coin';
import { type AllowedAccountTypes } from '_src/background/accounts-finder';

export interface UseAccountFinderOptions {
    accountType?: AllowedAccountTypes;
    coinType?: number;
    gasType?: string;
    accountGapLimit?: number;
    addressGapLimit?: number;
    sourceID: string;
}

export function useAccountsFinder({
    coinType = IOTA_COIN_TYPE_ID,
    gasType = GAS_TYPE_ARG,
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
            bip44CoinType: coinType,
            coinType: gasType,
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
