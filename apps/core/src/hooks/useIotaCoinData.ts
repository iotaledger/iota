// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';

import { useAppsBackend } from './useAppsBackend';

// TODO: We should consider using tRPC or something for apps-backend
type CoinData = {
    marketCap: string;
    fullyDilutedMarketCap: string;
    currentPrice: number;
    priceChangePercentageOver24H: number;
    circulatingSupply: number;
    totalSupply: number;
};

export const COIN_GECKO_IOTA_URL = 'https://www.coingecko.com/en/coins/iota';

export function useIotaCoinData() {
    const { request } = useAppsBackend();
    return useQuery({
        queryKey: ['iota-coin-data'],
        queryFn: () => request<CoinData>('coins/iota', {}),
        gcTime: 24 * 60 * 60 * 1000,
        staleTime: Infinity,
    });
}
