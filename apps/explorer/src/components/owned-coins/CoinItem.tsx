// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { KeyValueInfo } from '@iota/apps-ui-kit';
import { useFormatCoin } from '@iota/core';
import { type CoinStruct } from '@iota/iota-sdk/client';
import { formatAddress } from '@iota/iota-sdk/utils';

interface CoinItemProps {
    coin: CoinStruct;
}

export default function CoinItem({ coin }: CoinItemProps): JSX.Element {
    const [formattedBalance, symbol] = useFormatCoin(coin.balance, coin.coinType);
    return (
        <KeyValueInfo
            keyText={`${formattedBalance} ${symbol}`}
            valueLink={`/object/${coin.coinObjectId}`}
            valueText={formatAddress(coin.coinObjectId)}
            fullwidth
        />
    );
}
