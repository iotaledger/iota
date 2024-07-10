// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFormatCoin } from '@iota/core';
import React from 'react';

interface CoinItemProps {
    coinType: string;
    balance: bigint;
    onClick?: () => void;
}

function CoinItem({ coinType, balance, onClick }: CoinItemProps): React.JSX.Element {
    const [formattedCoin, coinSymbol, { data: coinMeta }] = useFormatCoin(balance, coinType);

    return (
        <div
            onClick={onClick}
            className="flex w-full cursor-pointer items-center justify-between gap-4 rounded border px-6 py-3"
        >
            <div className="flex flex-1 items-center justify-between gap-1.5">
                <div className="max-w-token-width">
                    <span className="truncate uppercase">{coinMeta?.name}</span>
                </div>
                <div className="flex flex-row items-center justify-center">
                    <span>
                        {formattedCoin} {coinSymbol}
                    </span>
                </div>
            </div>
        </div>
    );
}

export default CoinItem;
