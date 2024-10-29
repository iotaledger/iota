// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BalanceChange, CoinFormat, useFormatCoin, useCoinMetadata } from '@iota/core';
import { formatAddress } from '@iota/iota-sdk/utils';

interface BalanceChangeEntryProps {
    balanceChange: BalanceChange;
}

export default function BalanceChangeEntry({
    balanceChange: { amount, coinType, unRecognizedToken, owner },
}: BalanceChangeEntryProps) {
    const [formatted, symbol] = useFormatCoin(amount, coinType, CoinFormat.FULL);
    const { data: coinMetaData } = useCoinMetadata(coinType);

    const isPositive = BigInt(amount) > 0n;

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
            {owner && (
                <div className="flex w-full flex-row justify-between space-x-2 border-t pt-1">
                    <span>Owner</span>
                    <span>{formatAddress(owner)}</span>
                </div>
            )}
        </div>
    );
}
