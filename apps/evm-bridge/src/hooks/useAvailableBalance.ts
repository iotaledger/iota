import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { CoinFormat, useFormatCoin } from '@iota/core';
import { useSortedCoinsBalances } from './useSortedCoinsBalances';

export function useAvailableBalance(
    coinType: string = IOTA_TYPE_ARG,
    isFromLayer1: boolean = true,
): {
    availableBalance: bigint;
    isLoading: boolean;
    formattedAvailableBalance: string;
    symbol: string;
} {
    const { sortedCoinsBalanceL1, sortedCoinsBalanceL2, isLoadingL1, isLoadingL2 } =
        useSortedCoinsBalances();

    const sortedCoinsBalance = isFromLayer1 ? sortedCoinsBalanceL1 : sortedCoinsBalanceL2;

    const selectedCoinData = sortedCoinsBalance?.find((token) => token.coinType === coinType);

    const selectedCoinBalance = selectedCoinData?.totalBalance
        ? BigInt(selectedCoinData?.totalBalance)
        : 0n;

    const [formattedCoin, symbol] = useFormatCoin({
        balance: selectedCoinBalance,
        coinType,
        format: CoinFormat.FULL,
    });

    return {
        availableBalance: selectedCoinBalance,
        isLoading: isLoadingL1 || isLoadingL2,
        formattedAvailableBalance: formattedCoin,
        symbol,
    };
}
