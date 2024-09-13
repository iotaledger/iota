// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFormatCoin } from '@iota/core';
import { type CoinStruct } from '@iota/iota-sdk/client';
import { Text } from '@iota/ui';

import { ObjectLink } from '~/components/ui';

type CoinItemProps = {
    coin: CoinStruct;
};

export default function CoinItem({ coin }: CoinItemProps): JSX.Element {
    const [formattedBalance, symbol] = useFormatCoin(coin.balance, coin.coinType);
    return (
        <div className="flex items-center justify-between rounded-lg bg-white px-3 py-2 shadow-panel">
            <ObjectLink objectId={coin.coinObjectId} />
            <div className="col-span-3 inline-flex items-center gap-1">
                <Text color="steel-darker" variant="bodySmall/medium">
                    {formattedBalance}
                </Text>
                <Text color="steel" variant="subtitleSmallExtra/normal">
                    {symbol}
                </Text>
            </div>
        </div>
    );
}
