// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Heading } from '_src/ui/app/shared/heading';
import { Text } from '_src/ui/app/shared/text';
import { useFormatCoin } from '@iota/core';

interface TxnAmountProps {
    amount: string | number;
    coinType: string;
    label: string;
    approximation?: boolean;
}

// dont show amount if it is 0
// This happens when a user sends a transaction to self;
export function TxnAmount({ amount, coinType, label, approximation }: TxnAmountProps) {
    const [formatAmount, symbol] = useFormatCoin(Math.abs(Number(amount)), coinType);
    return Number(amount) !== 0 ? (
        <div className="flex w-full items-center justify-between py-3.5 first:pt-0">
            <Text variant="body" weight="medium" color="steel-darker">
                {label}
            </Text>
            <div className="flex items-center gap-1">
                <Heading variant="heading2" weight="semibold" color="gray-90">
                    {approximation ? '~' : ''}
                    {formatAmount}
                </Heading>
                <Text variant="body" weight="medium" color="steel-darker">
                    {symbol}
                </Text>
            </div>
        </div>
    ) : null;
}
