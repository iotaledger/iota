// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFormatCoin } from '@iota/core';

interface TransactionAmountProps {
    amount: string | number | bigint;
    coinType: string;
    label: string;
    approximation?: boolean;
}

export function TransactionAmount({
    amount,
    coinType,
    label,
    approximation,
}: TransactionAmountProps) {
    const [formatAmount, symbol] = useFormatCoin(Math.abs(Number(amount)), coinType);

    if (Number.isNaN(Number(amount))) return null;

    return (
        <div className="flex w-full items-center justify-between py-3.5 first:pt-0">
            {label}
            <div className="flex items-center gap-1">
                <h2>
                    {approximation ? '~' : ''}
                    {formatAmount}
                </h2>
                {symbol}
            </div>
        </div>
    );
}
