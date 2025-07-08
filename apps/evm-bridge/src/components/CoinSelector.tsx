// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useMemo, useRef } from 'react';
import { CoinSelector as CoreCoinSelector } from '@iota/core';
import { BridgeFormInputName } from '../lib/enums';
import { useFormContext } from 'react-hook-form';
import { DepositFormData } from '../lib/schema/bridgeForm.schema';
import { useSortedCoinsBalances } from '../hooks/useSortedCoinsBalances';

export function CoinSelector() {
    const { watch, setValue } = useFormContext<DepositFormData>();
    const { coinType: selectedCoinType, isFromLayer1 } = watch();

    const { sortedCoinsBalanceL1, sortedCoinsBalanceL2 } = useSortedCoinsBalances();

    // Track previous direction and coins count
    const previousCoinsCount = useRef(0);
    const previousDirection = useRef(isFromLayer1);

    const sortedCoins = useMemo(
        () => (isFromLayer1 ? sortedCoinsBalanceL1 : sortedCoinsBalanceL2),
        [isFromLayer1, sortedCoinsBalanceL1, sortedCoinsBalanceL2],
    );

    useEffect(() => {
        const coinsCountChanged = sortedCoins.length !== previousCoinsCount.current;
        const directionChanged = isFromLayer1 !== previousDirection.current;
        const shouldResetSelection = coinsCountChanged || directionChanged;

        // Update tracking variables for next comparison
        previousCoinsCount.current = sortedCoins.length;
        previousDirection.current = isFromLayer1;

        // Skip if no reset needed
        if (!shouldResetSelection) return;

        // Get first coin as default selection
        const firstCoinType = sortedCoins[0]?.coinType;

        // Skip if first coin matches current selection
        if (!firstCoinType || firstCoinType === selectedCoinType) return;

        // Reset selection to first coin
        setValue(BridgeFormInputName.CoinType, firstCoinType, {
            shouldValidate: true,
            shouldTouch: true,
        });
    }, [sortedCoins, isFromLayer1, setValue]);

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
