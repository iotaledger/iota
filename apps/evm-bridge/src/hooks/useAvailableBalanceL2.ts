import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useMemo } from 'react';
import { useAvailableIotaBalanceL2 } from './useAvailableIotaBalanceL2';
import { useGetAllBalancesL2 } from './useGetAllBalancesL2';
import { CoinFormat, useFormatCoin } from '@iota/core';
import { useAccount } from 'wagmi';

export function useAvailableBalanceL2(coinType: string = IOTA_TYPE_ARG): {
    availableBalance: bigint;
    isLoading: boolean;
    formattedAvailableBalance: string;
    symbol: string;
} {
    const addressL2 = useAccount().address as `0x${string}`;
    const selectedCoinType = coinType;
    // Fetch Layer 2 balance
    const {
        availableBalance: availableIotaBalance,
        formattedAvailableBalance: formattedIota,
        isLoading: isLoadingIota,
    } = useAvailableIotaBalanceL2();

    // Fetch Layer 2 balance for the selected coin type
    const { data: coinBalancesL2, isLoading: isLoadingCoin } = useGetAllBalancesL2(addressL2);

    const selectedCoinData = coinBalancesL2?.find((token) => token.coinType === selectedCoinType);

    const selectedCoinBalance = selectedCoinData?.totalBalance
        ? BigInt(selectedCoinData?.totalBalance)
        : 0n;

    const [formattedCoin, symbol] = useFormatCoin({
        balance: selectedCoinBalance,
        coinType: selectedCoinType,
        format: CoinFormat.FULL,
    });

    const isIotaCoinType = selectedCoinType === IOTA_TYPE_ARG;

    const result = useMemo(
        () => ({
            availableBalance: isIotaCoinType ? availableIotaBalance : selectedCoinBalance,
            isLoading: isIotaCoinType ? isLoadingIota : isLoadingCoin,
            formattedAvailableBalance: `${isIotaCoinType ? formattedIota : formattedCoin}`,
            symbol: symbol,
        }),
        [
            isIotaCoinType,
            availableIotaBalance,
            selectedCoinBalance,
            isLoadingIota,
            isLoadingCoin,
            formattedIota,
            formattedCoin,
            symbol,
        ],
    );

    return result;
}
