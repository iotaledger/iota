// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef } from 'react';
import { CoinSelector as CoreCoinSelector } from '@iota/core';
import { BridgeFormInputName } from '../lib/enums';
import { useFormContext } from 'react-hook-form';
import { DepositFormData } from '../lib/schema/bridgeForm.schema';
import { useSortedCoins } from '../hooks/useSortedCoins';

export function CoinSelector() {
    const { watch, setValue } = useFormContext<DepositFormData>();
    const { coinType: selectedCoinType, isFromLayer1 } = watch();

    const { sortedCoinsL1, sortedCoinsL2 } = useSortedCoins();

    // Track previous direction and coins count
    const previousCoinsCount = useRef(0);
    const previousDirection = useRef(isFromLayer1);

    const sortedCoins = isFromLayer1 ? sortedCoinsL1 : sortedCoinsL2;
    const sortedCoinsCoinTypes = sortedCoins.map((coin) => coin.coinType);

    useEffect(() => {
        const coinsCountChanged = sortedCoinsCoinTypes.length !== previousCoinsCount.current;
        const directionChanged = isFromLayer1 !== previousDirection.current;
        const shouldResetSelection = coinsCountChanged || directionChanged;

        // Update tracking variables for next comparison
        previousCoinsCount.current = sortedCoinsCoinTypes.length;
        previousDirection.current = isFromLayer1;

        // Skip if no reset needed
        if (!shouldResetSelection) return;

        // Find selected coin type in the sorted coins or default to the first one
        const coinTypeToSelect =
            sortedCoinsCoinTypes.find((coinType) => coinType === selectedCoinType) ||
            sortedCoinsCoinTypes[0];

        // Skip if first coin matches current selection
        if (!coinTypeToSelect || coinTypeToSelect === selectedCoinType) return;

        // Reset selection to first coin
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
