// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BalanceChange, CoinFormat, useFormatCoin, useCoinMetadata } from '@iota/core';

export default function BalanceChangeEntry({ change }: { change: BalanceChange }) {
    const { amount, coinType, unRecognizedToken } = change;
    const isPositive = BigInt(amount) > 0n;
    const [formatted, symbol] = useFormatCoin(amount, coinType, CoinFormat.FULL);
    const { data: coinMetaData } = useCoinMetadata(coinType);
    return (
        <div>
            <div>
                {coinMetaData?.name || symbol}
                {unRecognizedToken && <div>Unrecognized</div>}
            </div>
            <div>
                {isPositive ? '+' : ''}
                {formatted} {symbol}
            </div>
        </div>
    );
}
