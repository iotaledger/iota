// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFormatCoin, formatBalance, CoinFormat } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Heading, Text } from '@iota/ui';

type DelegationAmountProps = {
    amount: bigint | number | string;
    isStats?: boolean;
    inNano?: boolean;
};

export function DelegationAmount({
    amount,
    isStats,
    inNano = false,
}: DelegationAmountProps): JSX.Element {
    const [formattedAmount, symbol] = useFormatCoin(amount, IOTA_TYPE_ARG);
    const delegationAmount = inNano ? formatBalance(amount, 0, CoinFormat.FULL) : formattedAmount;
    const delegationSymbol = inNano ? 'nano' : symbol;
    return isStats ? (
        <div className="flex items-end gap-1.5 break-all">
            <Heading as="div" variant="heading3/semibold" color="steel-darker">
                {delegationAmount}
            </Heading>
            <Heading variant="heading4/medium" color="steel-darker">
                {delegationSymbol}
            </Heading>
        </div>
    ) : (
        <div className="flex h-full items-center gap-1">
            <div className="flex items-baseline gap-0.5 break-all text-steel-darker">
                <Text variant="body/medium">{delegationAmount}</Text>
                <Text variant="subtitleSmall/medium">{delegationSymbol}</Text>
            </div>
        </div>
    );
}
