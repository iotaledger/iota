// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Heading } from '_app/shared/heading';
import { Text } from '_app/shared/text';
import { useFormatCoin } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

//TODO unify StakeAmount and CoinBalance
interface StakeAmountProps {
    balance: bigint | number | string;
    variant: 'heading5' | 'body';
    isEarnedRewards?: boolean;
}

export function StakeAmount({ balance, variant, isEarnedRewards }: StakeAmountProps) {
    const [formatted, symbol] = useFormatCoin(balance, IOTA_TYPE_ARG);
    // Handle case of 0 balance
    const zeroBalanceColor = !!balance;
    const earnRewardColor = isEarnedRewards && (zeroBalanceColor ? 'success-dark' : 'gray-60');
    const colorAmount = variant === 'heading5' ? 'gray-90' : 'steel-darker';
    const colorSymbol = variant === 'heading5' ? 'steel' : 'steel-darker';

    return (
        <div className="flex flex-nowrap items-baseline gap-0.5 align-baseline">
            {variant === 'heading5' ? (
                <Heading
                    variant="heading5"
                    as="div"
                    weight="semibold"
                    color={earnRewardColor || colorAmount}
                >
                    {formatted}
                </Heading>
            ) : (
                <Text variant={variant} weight="semibold" color={earnRewardColor || colorAmount}>
                    {formatted}
                </Text>
            )}

            <Text
                variant={variant === 'heading5' ? 'bodySmall' : 'body'}
                color={earnRewardColor || colorSymbol}
                weight="medium"
            >
                {symbol}
            </Text>
        </div>
    );
}
