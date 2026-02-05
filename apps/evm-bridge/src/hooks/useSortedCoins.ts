// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFeatureValue } from '@growthbook/growthbook-react';
import { Feature, useGetAllBalances, useSortedCoinsByCategories } from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useAvailableIotaBalanceL1 } from './useAvailableIotaBalanceL1';
import { useAvailableIotaBalanceL2 } from './useAvailableIotaBalanceL2';
import { useGetAllBalancesL2 } from './useGetAllBalancesL2';
import { CoinBalance } from '@iota/iota-sdk/client';
import { useAccount } from 'wagmi';

export const useSortedCoins = () => {
    const addressL1 = useCurrentAccount()?.address as string;
    const addressL2 = useAccount().address as `0x${string}`;

    const knownEvmCoins = useFeatureValue(Feature.KnownIotaEVMCoinTypes, []);

    // Fetch L1 balances
    const { availableBalance: availableIotaBalanceL1, isLoading: isLoadingIotaL1 } =
        useAvailableIotaBalanceL1();

    const { data: coinsBalanceL1, isLoading: isLoadingAllBalancesL1 } =
        useGetAllBalances(addressL1);
    const { recognized: recognizedL1, pinned: pinnedL1 } = useSortedCoinsByCategories(
        coinsBalanceL1 || [],
        knownEvmCoins,
    );
    let sortedCoinsL1 = [...recognizedL1, ...pinnedL1];
    // If the sorted coins array is empty, add IOTA with zero balance
    if (sortedCoinsL1.length === 0) {
        sortedCoinsL1 = [
            {
                coinType: IOTA_TYPE_ARG,
                totalBalance: '0',
                coinObjectCount: 0,
            },
        ];
    }

    // Fetch L2 balances
    const { availableBalance: availableIotaBalanceL2, isLoading: isLoadingIotaL2 } =
        useAvailableIotaBalanceL2();

    const { data: coinsBalancesL2, isLoading: isLoadingAllBalancesL2 } =
        useGetAllBalancesL2(addressL2);
    const { recognized: recognizedL2, pinned: pinnedL2 } = useSortedCoinsByCategories(
        coinsBalancesL2 || [],
        knownEvmCoins,
    );
    const sortedCoinsL2 = [...recognizedL2, ...pinnedL2];

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
    // Adjust the iota balances for both L1 and L2 balances. Add available iota instead of total balance
    const adjustedSortedCoinsL1 = adjustIotaBalance(sortedCoinsL1, availableIotaBalanceL1);
    const adjustedSortedCoinsL2 = adjustIotaBalance(sortedCoinsL2, availableIotaBalanceL2);

    return {
        sortedCoinsL1: adjustedSortedCoinsL1,
        sortedCoinsL2: adjustedSortedCoinsL2,
        isLoadingL1: isLoadingIotaL1 || isLoadingAllBalancesL1,
        isLoadingL2: isLoadingIotaL2 || isLoadingAllBalancesL2,
    };
};
