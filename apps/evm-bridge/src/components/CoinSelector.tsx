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

    // Track previous list state
    const prevListLengthRef = useRef(0);
    const prevIsFromLayer1Ref = useRef(isFromLayer1);

    const sortedCoinsBalance = useMemo(
        () => (isFromLayer1 ? sortedCoinsBalanceL1 : sortedCoinsBalanceL2),
        [isFromLayer1, sortedCoinsBalanceL1, sortedCoinsBalanceL2],
    );

    useEffect(() => {
        const currentLength = sortedCoinsBalance.length;
        const lengthChanged = currentLength !== prevListLengthRef.current;
        const directionChanged = isFromLayer1 !== prevIsFromLayer1Ref.current;

        // Only reset selection when list contents change or direction changes
        if ((lengthChanged && currentLength > 0) || directionChanged) {
            // Only update if there are coins to select and the desired coin exists
            if (currentLength > 0 && sortedCoinsBalance[0]) {
                const firstCoinType = sortedCoinsBalance[0].coinType;

                // Only update if different from current selection
                if (selectedCoinType !== firstCoinType) {
                    setValue(BridgeFormInputName.CoinType, firstCoinType, {
                        shouldValidate: true,
                        shouldTouch: true,
                    });
                }
            }
        }

        // Update refs for next comparison
        prevListLengthRef.current = currentLength;
        prevIsFromLayer1Ref.current = isFromLayer1;
    }, [sortedCoinsBalance, isFromLayer1, setValue]);

    return (
        <CoreCoinSelector
            activeCoinType={selectedCoinType}
            coins={sortedCoinsBalance}
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
