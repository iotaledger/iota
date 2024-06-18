// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BalanceChange, CoinFormat, useFormatCoin, useCoinMetadata } from '@iota/core';
import { formatAddress } from '@iota/iota.js/utils';

interface BalanceChangeEntryProps {
    balanceChange: BalanceChange;
}

export default function BalanceChangeEntry({ balanceChange }: BalanceChangeEntryProps) {
    const { amount, coinType, unRecognizedToken } = balanceChange;
    const isPositive = BigInt(amount) > 0n;
    const [formatted, symbol] = useFormatCoin(amount, coinType, CoinFormat.FULL);
    const { data: coinMetaData } = useCoinMetadata(coinType);
    return (
        <div className="flex flex-col py-2">
            <div className="flex flex-row justify-between space-x-2 py-2">
                <div className="flex flex-row items-center gap-2">
                    <span className="font-medium">{coinMetaData?.name || symbol}</span>
                    {unRecognizedToken && <div>Unrecognized</div>}
                </div>
                <div className={`flex flex-row gap-2 ${!isPositive ? 'text-red-600' : ''}`}>
                    {isPositive && <>+</>}
                    {formatted} {symbol}
                </div>
            </div>
            {balanceChange.owner && (
                <div className="flex w-full flex-row justify-between space-x-2 border-t pt-1">
                    <span>Owner</span>
                    <span>{formatAddress(balanceChange.owner)}</span>
                </div>
            )}
        </div>
    );
}
