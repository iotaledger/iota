// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from 'react';
import { CoinSelector as CoreCoinSelector } from '@iota/core';
import { BridgeFormInputName } from '../lib/enums';
import { useFormContext } from 'react-hook-form';
import { DepositFormData } from '../lib/schema/bridgeForm.schema';
import { useSortedCoins } from '../hooks/useSortedCoins';
import { ampli, BridgeDirection } from '../shared/analytics';
import { useCoinsMetadata } from '../hooks/useCoinsMetadata';

export function CoinSelector() {
    const { watch, setValue } = useFormContext<DepositFormData>();
    const { coinType: selectedCoinType, isFromLayer1 } = watch();

    const { sortedCoinsL1, sortedCoinsL2 } = useSortedCoins();
    const sortedCoins = isFromLayer1 ? sortedCoinsL1 : sortedCoinsL2;
    const { metadata } = useCoinsMetadata(sortedCoins);
    const sortedCoinsCoinTypes = sortedCoins.map((coin) => coin.coinType);

    useEffect(() => {
        // Find selected coin type in the sorted coins or default to the first one
        const coinTypeToSelect =
            sortedCoinsCoinTypes.find((coinType) => coinType === selectedCoinType) ||
            sortedCoinsCoinTypes[0];

        if (!coinTypeToSelect || coinTypeToSelect === selectedCoinType) return;

        setValue(BridgeFormInputName.CoinType, coinTypeToSelect, {
            shouldValidate: true,
            shouldTouch: true,
        });
    }, [JSON.stringify(sortedCoinsCoinTypes), isFromLayer1, setValue]);

    return (
        <CoreCoinSelector
            activeCoinType={selectedCoinType}
            coins={sortedCoins}
            onClick={(coinType: string) => {
                const coinMetadata = metadata[coinType];
                setValue(BridgeFormInputName.DepositAmount, '', {
                    shouldValidate: true,
                    shouldTouch: true,
                });
                setValue(BridgeFormInputName.CoinType, coinType, {
                    shouldValidate: true,
                    shouldTouch: true,
                });
                ampli.selectedCoin({
                    bridgeDirection: isFromLayer1 ? BridgeDirection.L1ToL2 : BridgeDirection.L2ToL1,
                    coinSymbol: coinMetadata?.symbol || 'unknown',
                    coinType: coinType,
                });
            }}
        />
    );
}
