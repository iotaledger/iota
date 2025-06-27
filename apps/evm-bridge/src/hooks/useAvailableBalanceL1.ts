import { useCurrentAccount } from '@iota/dapp-kit';
import { useBalance as useBalanceL1 } from './useBalance';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useAvailableIotaBalanceL1 } from './useAvailableIotaBalanceL1';
import { CoinFormat, useFormatCoin } from '@iota/core';
import { useMemo } from 'react';

export function useAvailableBalanceL1(): {
    availableBalance: bigint;
    isLoading: boolean;
    formattedAvailableBalance: string;
    symbol: string;
} {
    const layer1Account = useCurrentAccount();
    const selectedCoinType = IOTA_TYPE_ARG;
    // Fetch Layer 1 balance
    const {
        availableBalance: availableIotaBalance,
        formattedAvailableBalance: formattedIota,
        isLoading: isLoadingIota,
    } = useAvailableIotaBalanceL1();

    // Fetch Layer 1 balance for the selected coin type
    const { data: selectedCoinData, isLoading: isLoadingCoin } = useBalanceL1(
        layer1Account?.address as `0x${string}`,
        undefined,
        selectedCoinType,
    );

    const cionBalance = selectedCoinData?.totalBalance
        ? BigInt(selectedCoinData?.totalBalance)
        : 0n;

    const [formattedCoin, symbol] = useFormatCoin({
        balance: cionBalance,
        coinType: selectedCoinType,
        format: CoinFormat.FULL,
    });

    const isIotaCoinType = selectedCoinType === IOTA_TYPE_ARG;

    // Compute final values
    const result = useMemo(
        () => ({
            availableBalance: isIotaCoinType ? availableIotaBalance : cionBalance,
            isLoading: isIotaCoinType ? isLoadingIota : isLoadingCoin,
            formattedAvailableBalance: `${isIotaCoinType ? formattedIota : formattedCoin}`,
            symbol: symbol,
        }),
        [
            isIotaCoinType,
            availableIotaBalance,
            cionBalance,
            isLoadingIota,
            isLoadingCoin,
            formattedIota,
            formattedCoin,
            symbol,
        ],
    );

    return result;
}
