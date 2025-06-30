// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinSelector as CoreCoinSelector } from '@iota/core';
import { BridgeFormInputName } from '../lib/enums';
import { useFormContext } from 'react-hook-form';
import { DepositFormData } from '../lib/schema/bridgeForm.schema';
import { useSortedCoinsBalances } from '../hooks/useSortedCoinsBalances';

export function CoinSelector() {
    const { watch, setValue } = useFormContext<DepositFormData>();
    const { coinType: selectedCoinType, isFromLayer1 } = watch();

    const { sortedCoinsBalanceL1, sortedCoinsBalanceL2 } = useSortedCoinsBalances();

    const sortedCoinsBalance = isFromLayer1 ? sortedCoinsBalanceL1 : sortedCoinsBalanceL2;
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
