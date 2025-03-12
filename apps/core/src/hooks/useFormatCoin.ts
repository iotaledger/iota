// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { CoinMetadata } from '@iota/iota-sdk/client';
import { IOTA_DECIMALS, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useQuery, type UseQueryResult } from '@tanstack/react-query';
import BigNumber from 'bignumber.js';
import { useMemo } from 'react';

import { formatAmount } from '../utils/formatAmount';

type FormattedCoin = [
    formattedBalance: string,
    coinSymbol: string,
    queryResult: UseQueryResult<CoinMetadata | null>,
];

export enum CoinFormat {
    ROUNDED = 'ROUNDED',
    FULL = 'FULL',
}

/**
 * Formats a coin balance based on our standard coin display logic.
 * If the balance is less than 1, it will be displayed in its full decimal form.
 * For values greater than 1, it will be truncated to 3 decimal places.
 */
export function formatBalance(
    balance: bigint | number | string,
    decimals: number,
    format: CoinFormat = CoinFormat.ROUNDED,
    showSign = false,
) {
    const bn = new BigNumber(balance.toString()).shiftedBy(-1 * decimals);
    let formattedBalance = formatAmount(bn);

    if (format === CoinFormat.FULL) {
        formattedBalance = bn.toFormat();
    }

    if (showSign && !formattedBalance.startsWith('-')) {
        formattedBalance = `+${formattedBalance}`;
    }

    return formattedBalance;
}

const ELLIPSIS = '\u{2026}';
const SYMBOL_TRUNCATE_LENGTH = 5;
const NAME_TRUNCATE_LENGTH = 10;

export function useCoinMetadata(coinType?: string | null) {
    const client = useIotaClient();
    return useQuery({
        queryKey: ['coin-metadata', coinType],
        queryFn: async () => {
            if (!coinType) {
                throw new Error(
                    'Fetching coin metadata should be disabled when coin type is disabled.',
                );
            }

            // Optimize the known case of IOTA to avoid a network call:
            if (coinType === IOTA_TYPE_ARG) {
                const metadata: CoinMetadata = IOTA_COIN_METADATA;

                return metadata;
            }

            return client.getCoinMetadata({ coinType });
        },
        select(data) {
            if (!data) return null;

            return {
                ...data,
                symbol:
                    data.symbol.length > SYMBOL_TRUNCATE_LENGTH
                        ? data.symbol.slice(0, SYMBOL_TRUNCATE_LENGTH) + ELLIPSIS
                        : data.symbol,
                name:
                    data.name.length > NAME_TRUNCATE_LENGTH
                        ? data.name.slice(0, NAME_TRUNCATE_LENGTH) + ELLIPSIS
                        : data.name,
            };
        },
        retry: false,
        enabled: !!coinType,
        staleTime: Infinity,
        gcTime: 24 * 60 * 60 * 1000,
    });
}

export const IOTA_COIN_METADATA: CoinMetadata = {
    id: null,
    decimals: IOTA_DECIMALS,
    description: '',
    iconUrl: null,
    name: 'IOTA',
    symbol: 'IOTA',
};

interface FormatCoinOptions {
    balance?: bigint | number | string | null;
    coinType?: string;
    format?: CoinFormat;
    showSign?: boolean;
}
// TODO #1: This handles undefined values to make it easier to integrate with
// the reset of the app as it is today, but it really shouldn't in a perfect world.
export function useFormatCoin({
    balance,
    coinType = IOTA_TYPE_ARG,
    format = CoinFormat.ROUNDED,
    showSign = false,
}: FormatCoinOptions): FormattedCoin {
    const fallbackSymbol = useMemo(
        () => (coinType ? (getCoinSymbol(coinType) ?? '') : ''),
        [coinType],
    );
    const queryResult = useCoinMetadata(coinType);
    const { isFetched, data } = queryResult;

    const formatted = useMemo(() => {
        if (typeof balance === 'undefined' || balance === null) return '';

        if (!isFetched) return '...';

        return formatBalance(balance, data?.decimals ?? 0, format, showSign);
    }, [data?.decimals, isFetched, balance, format]);

    return [formatted, isFetched ? data?.symbol || fallbackSymbol : '', queryResult];
}

/** @deprecated use coin metadata instead */
export function getCoinSymbol(coinTypeArg: string) {
    return coinTypeArg.substring(coinTypeArg.lastIndexOf(':') + 1);
}
