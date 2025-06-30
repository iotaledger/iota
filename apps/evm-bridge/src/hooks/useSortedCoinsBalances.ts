import { useFeatureValue } from '@growthbook/growthbook-react';
import { Feature, useGetAllBalances, useSortedCoinsByCategories } from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useAvailableIotaBalanceL1 } from './useAvailableIotaBalanceL1';
import { useAvailableIotaBalanceL2 } from './useAvailableIotaBalanceL2';
import { useGetAllBalancesL2 } from './useGetAllBalancesL2';
import { CoinBalance } from '@iota/iota-sdk/client';

export const useSortedCoinsBalances = () => {
    const address = useCurrentAccount()?.address as string;

    const knownEvmCoins = useFeatureValue(Feature.KnownIotaEVMCoinTypes, []);

    const { availableBalance: availableIotaBalanceL1 } = useAvailableIotaBalanceL1();

    const { data: coinsBalanceL1 } = useGetAllBalances(address);
    const { recognized: recognizedL1, pinned: pinnedL1 } = useSortedCoinsByCategories(
        coinsBalanceL1 || [],
        knownEvmCoins,
    );
    const sortedCoinsBalanceL1 = [...recognizedL1, ...pinnedL1];

    // Fetch L2 balance for L1 address
    const { availableBalance: availableIotaBalanceL2 } = useAvailableIotaBalanceL2();

    const { data: l1AddressCoinsBalanceInL2 } = useGetAllBalancesL2(address);
    const { recognized: recognizedL2, pinned: pinnedL2 } = useSortedCoinsByCategories(
        l1AddressCoinsBalanceInL2 || [],
        knownEvmCoins,
    );
    const sortedCoinsBalanceL2 = [...recognizedL2, ...pinnedL2];

    // Function to adjust IOTA balance in the coins
    const adjustIotaBalance = (
        coins: CoinBalance[],
        availableBalance: bigint | null | undefined,
    ): CoinBalance[] => {
        return coins.map((coin) =>
            coin.coinType === IOTA_TYPE_ARG
                ? {
                      ...coin,
                      totalBalance: availableBalance?.toString() ?? coin.totalBalance,
                  }
                : coin,
        );
    };
    // Adjust the iota balances to both L1 and L2 balances. Add available iota instead of total balance
    const updatedSortedCoinsBalanceL1 = adjustIotaBalance(
        sortedCoinsBalanceL1,
        availableIotaBalanceL1,
    );
    const updatedSortedCoinsBalanceL2 = adjustIotaBalance(
        sortedCoinsBalanceL2,
        availableIotaBalanceL2,
    );

    return {
        sortedCoinsBalanceL1: updatedSortedCoinsBalanceL1,
        sortedCoinsBalanceL2: updatedSortedCoinsBalanceL2,
    };
};
