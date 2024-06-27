// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { filterAndSortTokenBalances } from '@iota/core';
import { useCurrentAccount, useIotaClientQuery } from '@iota/dapp-kit';
import { CoinItem, SendCoinPopup } from '@/components';
import { usePopups } from '@/hooks';
import { CoinBalance } from '@iota/iota.js/client';

function AllCoins(): React.JSX.Element {
    const { openPopup, closePopup } = usePopups();
    const account = useCurrentAccount();
    const { data: coins } = useIotaClientQuery(
        'getAllBalances',
        { owner: account?.address ?? '' },
        {
            enabled: !!account?.address,
            select: filterAndSortTokenBalances,
        },
    );

    const openSendTokenPopup = (coin: CoinBalance, address: string) => {
        openPopup(<SendCoinPopup coin={coin} senderAddress={address} onClose={closePopup} />);
    };

    return (
        <div className="flex w-2/3 flex-col items-center space-y-2">
            <h3>My Coins:</h3>
            {coins?.map((coin, index) => {
                return (
                    <CoinItem
                        key={index}
                        coinType={coin.coinType}
                        balance={BigInt(coin.totalBalance)}
                        onClick={() => openSendTokenPopup(coin, account?.address ?? '')}
                    />
                );
            })}
        </div>
    );
}

export default AllCoins;
