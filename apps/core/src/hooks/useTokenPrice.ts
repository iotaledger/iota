// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';
import BigNumber from 'bignumber.js';

import { useAppsBackend } from './useAppsBackend';
import { useCoinMetadata } from './useFormatCoin';
import { Feature, FiatTokenName } from '../enums';
import { COIN_TYPE_TO_FIAT_TOKEN_NAME } from '../constants';
import { Network } from '@iota/iota-sdk/client';
import { useFeatureEnabledByNetwork } from './useFeatureEnabledByNetwork';

type TokenPriceResponse = { price: string | null };

export function useTokenPrice(tokenName: FiatTokenName | null, network: Network) {
    const { request } = useAppsBackend();
    const isFiatConversionEnabled = useFeatureEnabledByNetwork(Feature.FiatConversion, network);
    return useQuery({
        queryKey: ['apps-backend', 'token-price', isFiatConversionEnabled, network, tokenName],
        queryFn: () => {
            if (!isFiatConversionEnabled || !tokenName) return { price: null };
            return request<TokenPriceResponse>(`coin-price/${tokenName}`);
        },

        // These values are set to one minute to prevent displaying stale data, as token prices can change frequently.
        staleTime: 60 * 1000,
        gcTime: 60 * 1000,
    });
}

export function useBalanceInUSD(
    coinType: string,
    balance: bigint | string | number,
    network: Network,
) {
    const { data: coinMetadata } = useCoinMetadata(coinType);
    const tokenName: FiatTokenName | null = COIN_TYPE_TO_FIAT_TOKEN_NAME[coinType];
    const { data: tokenPrice } = useTokenPrice(tokenName, network);
    if (!tokenPrice || !coinMetadata || !tokenPrice.price) return null;
    return new BigNumber(balance.toString())
        .shiftedBy(-1 * coinMetadata.decimals)
        .multipliedBy(tokenPrice.price)
        .toNumber();
}
