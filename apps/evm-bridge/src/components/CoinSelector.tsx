// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount } from '@iota/dapp-kit';
import {
    CoinSelector as CoreCoinSelector,
    Feature,
    useGetAllBalances,
    useSortedCoinsByCategories,
} from '@iota/core';
import { useFeatureValue } from '@growthbook/growthbook-react';
import { BridgeFormInputName } from '../lib/enums';
import { useFormContext } from 'react-hook-form';
import { DepositFormData } from '../lib/schema/bridgeForm.schema';
import { useGetAllBalancesL2 } from '../hooks/useGetAllBalancesL2';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useAvailableIotaBalanceL1 } from '../hooks/useAvailableIotaBalanceL1';
import { useAvailableIotaBalanceL2 } from '../hooks/useAvailableIotaBalanceL2';

export function CoinSelector() {
    const address = useCurrentAccount()?.address as string;

    const { watch, setValue } = useFormContext<DepositFormData>();
    const { coinType: selectedCoinType, isFromLayer1 } = watch();

    const knownEvmCoins = useFeatureValue(Feature.KnownIotaEVMCoinTypes, []);

    const { data: coinsBalanceL1 } = useGetAllBalances(address);
    const { availableBalance: availableIotaBalanceL1 } = useAvailableIotaBalanceL1();

    const { recognized: recognizedL1, pinned: pinnedL1 } = useSortedCoinsByCategories(
        coinsBalanceL1 || [],
        knownEvmCoins,
    );
    const sortedCoinsBalanceL1 = [...recognizedL1, ...pinnedL1];

    // Fetch L2 balance for L1 address
    const { data: l1AddressCoinsBalanceInL2 } = useGetAllBalancesL2(address);
    const { recognized: recognizedL2, pinned: pinnedL2 } = useSortedCoinsByCategories(
        l1AddressCoinsBalanceInL2 || [],
        knownEvmCoins,
    );
    const sortedCoinsBalanceL2 = [...recognizedL2, ...pinnedL2];

    const { availableBalance: availableIotaBalanceL2 } = useAvailableIotaBalanceL2();

    // adjust L1 iota total Balance in sortedCoinsBalanceL1 to available balance
    const updatedSortedCoinsBalanceL1 = sortedCoinsBalanceL1.map((coin) => {
        if (coin.coinType === IOTA_TYPE_ARG) {
            return {
                ...coin,
                totalBalance: availableIotaBalanceL1
                    ? availableIotaBalanceL1.toString()
                    : coin.totalBalance,
            };
        }
        return coin;
    });

    // adjust L@ iota total Balance in sortedCoinsBalanceL2 to available balance
    const updatedSortedCoinsBalanceL2 = sortedCoinsBalanceL2.map((coin) => {
        if (coin.coinType === IOTA_TYPE_ARG) {
            return {
                ...coin,
                totalBalance: availableIotaBalanceL2
                    ? availableIotaBalanceL2.toString()
                    : coin.totalBalance,
            };
        }
        return coin;
    });

    const sortedCoinsBalance = isFromLayer1
        ? updatedSortedCoinsBalanceL1
        : updatedSortedCoinsBalanceL2;
    return (
        <CoreCoinSelector
            activeCoinType={selectedCoinType}
            coins={sortedCoinsBalance}
            onClick={(coinType) => {
                setValue(BridgeFormInputName.DepositAmount, '', {
                    shouldValidate: true,
                    shouldTouch: true,
                });
                setValue(BridgeFormInputName.CoinType, coinType, {
                    shouldValidate: true,
                    shouldTouch: true,
                });
            }}
        />
    );
}
