// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { filterAndSortTokenBalances } from '@iota/core';
import { useCurrentAccount, useIotaClientQuery } from '@iota/dapp-kit';
import { CoinItem, SendCoinButton } from '@/components';

function AllCoins(): React.JSX.Element {
    const account = useCurrentAccount();
    const { data: coins } = useIotaClientQuery(
        'getAllBalances',
        { owner: account?.address ?? '' },
        {
            enabled: !!account?.address,
            select: filterAndSortTokenBalances,
        },
    );

    return (
        <div className="flex w-full flex-col items-center space-y-2">
            <h3>Coins:</h3>
            {coins?.map((coin, index) => {
                return (
                    <CoinItem
                        key={index}
                        coinType={coin.coinType}
                        balance={BigInt(coin.totalBalance)}
                        action={
                            account?.address && (
                                <SendCoinButton address={account.address} coin={coin} />
                            )
                        }
                    />
                );
            })}
        </div>
    );
}

export default AllCoins;
