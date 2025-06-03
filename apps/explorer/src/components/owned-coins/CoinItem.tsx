// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { KeyValueInfo } from '@iota/apps-ui-kit';
import { CoinFormat, useFormatCoin } from '@iota/core';
import { type CoinStruct } from '@iota/iota-sdk/client';
import { formatAddress } from '@iota/iota-sdk/utils';
import { ObjectLink } from '../ui';
import { onCopySuccess } from '~/lib/utils';

interface CoinItemProps {
    coin: CoinStruct;
}

export function CoinItem({ coin }: CoinItemProps): JSX.Element {
    const [formattedBalance, symbol] = useFormatCoin({
        balance: coin.balance,
        coinType: coin.coinType,
        format: CoinFormat.FULL,
    });
    return (
        <KeyValueInfo
            keyText={`${formattedBalance} ${symbol}`}
            isReverse
            value={
                <ObjectLink
                    objectId={coin.coinObjectId}
                    label={formatAddress(coin.coinObjectId)}
                    copyText={coin.coinObjectId}
                    onCopySuccess={onCopySuccess}
                />
            }
            fullwidth
        />
    );
}
