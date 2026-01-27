// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinFormat, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useFormatCoin } from '@iota/core';
import { useSortedCoins } from './useSortedCoins';

export function useAvailableBalance(
    coinType: string = IOTA_TYPE_ARG,
    isFromLayer1: boolean = true,
): {
    availableBalance: bigint;
    isLoading: boolean;
    formattedAvailableBalance: string;
    symbol: string;
} {
    const { sortedCoinsL1, sortedCoinsL2, isLoadingL1, isLoadingL2 } = useSortedCoins();

    const sortedCoins = isFromLayer1 ? sortedCoinsL1 : sortedCoinsL2;

    const selectedCoinData = sortedCoins?.find((token) => token.coinType === coinType);

    const selectedCoinBalance = selectedCoinData?.totalBalance
        ? BigInt(selectedCoinData?.totalBalance)
        : 0n;

    const [formattedCoin, symbol] = useFormatCoin({
        balance: selectedCoinBalance,
        coinType,
        format: CoinFormat.Full,
    });

    return {
        availableBalance: selectedCoinBalance,
        isLoading: isLoadingL1 || isLoadingL2,
        formattedAvailableBalance: formattedCoin,
        symbol,
    };
}
