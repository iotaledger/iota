// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFormatCoin, CoinFormat, formatBalance } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Text } from '@iota/ui';

type StakeColumnProps = {
    stake: bigint | number | string;
    hideCoinSymbol?: boolean;
    inMICROS?: boolean;
};

export function StakeColumn({
    stake,
    hideCoinSymbol,
    inMICROS = false,
}: StakeColumnProps): JSX.Element {
    const coinFormat = hideCoinSymbol ? CoinFormat.FULL : CoinFormat.ROUNDED;
    const [amount, symbol] = useFormatCoin(stake, IOTA_TYPE_ARG, coinFormat);
    return (
        <div className="flex items-end gap-0.5">
            <Text variant="bodySmall/medium" color="steel-darker">
                {inMICROS ? formatBalance(stake, 0, coinFormat) : amount}
            </Text>
            {!hideCoinSymbol && (
                <Text variant="captionSmall/medium" color="steel-dark">
                    {inMICROS ? 'MICROS' : symbol}
                </Text>
            )}
        </div>
    );
}
