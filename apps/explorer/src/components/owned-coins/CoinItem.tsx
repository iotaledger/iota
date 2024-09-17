// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { KeyValueInfo } from '@iota/apps-ui-kit';
import { useFormatCoin } from '@iota/core';
import { type CoinStruct } from '@iota/iota-sdk/client';
import { formatAddress } from '@iota/iota-sdk/utils';
import { ObjectLink } from '../ui';

interface CoinItemProps {
    coin: CoinStruct;
}

export default function CoinItem({ coin }: CoinItemProps): JSX.Element {
    const [formattedBalance, symbol] = useFormatCoin(coin.balance, coin.coinType);
    return (
        <ObjectLink objectId={coin.coinObjectId}>
            <KeyValueInfo
                keyText={`${formattedBalance} ${symbol}`}
                valueText={formatAddress(coin.coinObjectId)}
                fullwidth
            />
        </ObjectLink>
    );
}
