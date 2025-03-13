// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TableCellText } from '@iota/apps-ui-kit';
import { useFormatCoin, CoinFormat, formatBalance } from '@iota/core';

type StakeColumnProps = {
    stake: bigint | number | string;
    hideCoinSymbol?: boolean;
    inNano?: boolean;
};

export function StakeColumn({
    stake,
    hideCoinSymbol,
    inNano = false,
}: StakeColumnProps): JSX.Element {
    const coinFormat = hideCoinSymbol ? CoinFormat.FULL : CoinFormat.ROUNDED;
    const [amount, symbol] = useFormatCoin({ balance: stake, format: coinFormat });

    const label = inNano ? formatBalance(stake, 0, coinFormat) : amount;
    const supportingLabel = inNano ? 'nano' : symbol;

    return (
        <span className="whitespace-nowrap">
            <TableCellText supportingLabel={supportingLabel}>{label}</TableCellText>
        </span>
    );
}
