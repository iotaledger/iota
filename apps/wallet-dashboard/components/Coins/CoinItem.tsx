// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFormatCoin } from '@iota/core';
import { type ReactNode } from 'react';

type CoinItemProps = {
    coinType: string;
    balance: bigint;
    action?: ReactNode;
};

function CoinItem({ coinType, balance, action }: CoinItemProps): JSX.Element {
    const [formattedCoin, coinSymbol, { data: coinMeta }] = useFormatCoin(balance, coinType);

    return (
        <div className="flex w-full items-center justify-between gap-4 rounded border px-6 py-3">
            <div className="flex flex-1 items-center justify-between gap-1.5">
                <div className="max-w-token-width">
                    <span className="truncate uppercase">{coinMeta?.name}</span>
                </div>
                <div className="flex flex-row items-center justify-center">
                    <span>
                        {formattedCoin} {coinSymbol}
                    </span>
                </div>
                {action}
            </div>
        </div>
    );
}

export default CoinItem;
