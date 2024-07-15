// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { GAS_COIN_TYPE } from '_redux/slices/iota-objects/Coin';
import { Text } from '_src/ui/app/shared/text';
import { useFormatCoin } from '@iota/core';

interface TxnGasSummaryProps {
    totalGas: bigint;
    transferAmount: bigint | null;
}

//TODO add gas breakdown
export function TxnGasSummary({ totalGas, transferAmount }: TxnGasSummaryProps) {
    const [totalAmount, totalAmountSymbol] = useFormatCoin(
        totalGas + (transferAmount || 0n),
        GAS_COIN_TYPE,
    );
    const [gas, symbol] = useFormatCoin(totalGas, GAS_COIN_TYPE);

    return (
        <div className="flex w-full flex-col items-center gap-3.5 border-x-0 border-b-0 border-t border-solid border-steel/20 py-3.5 first:pt-0">
            <div className="flex w-full items-center justify-between">
                <Text variant="body" weight="medium" color="steel-darker">
                    Gas Fees
                </Text>
                <Text variant="body" weight="medium" color="steel-darker">
                    {gas} {symbol}
                </Text>
            </div>
            {transferAmount ? (
                <div className="flex w-full items-center justify-between">
                    <Text variant="body" weight="medium" color="steel-darker">
                        Total Amount
                    </Text>
                    <Text variant="body" weight="medium" color="steel-darker">
                        {totalAmount} {totalAmountSymbol}
                    </Text>
                </div>
            ) : null}
        </div>
    );
}
