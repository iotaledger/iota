// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';
import BigNumber from 'bignumber.js';

import { useAppsBackend } from './useAppsBackend';
import { useCoinMetadata } from './useFormatCoin';
import { FiatTokenName } from '../enums';
import { COIN_TYPE_TO_FIAT_TOKEN_NAME } from '../constants/coinTypeToFiatTokenName.constants';

type TokenPriceResponse = { price: string | null };

export function useTokenPrice(tokenName: FiatTokenName) {
    const { request } = useAppsBackend();
    return useQuery({
        queryKey: ['apps-backend', 'token-price', tokenName],
        queryFn: () => request<TokenPriceResponse>(`coin-price/${tokenName}`),

        // These values are set to one minute to prevent displaying stale data, as token prices can change frequently.
        staleTime: 60 * 1000,
        gcTime: 60 * 1000,
    });
}

export function useBalanceInUSD(coinType: string, balance: bigint | string | number) {
    const { data: coinMetadata } = useCoinMetadata(coinType);
    const tokenName = COIN_TYPE_TO_FIAT_TOKEN_NAME[coinType];
    if (!tokenName) return null;
    const { data: tokenPrice } = useTokenPrice(tokenName);
    if (!tokenPrice || !coinMetadata || !tokenPrice.price) return null;
    return new BigNumber(balance.toString())
        .shiftedBy(-1 * coinMetadata.decimals)
        .multipliedBy(tokenPrice.price)
        .toNumber();
}
