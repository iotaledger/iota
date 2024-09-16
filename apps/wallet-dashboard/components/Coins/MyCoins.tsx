// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { useCurrentAccount, useIotaClientQuery } from '@iota/dapp-kit';
import { CoinItem, SendCoinPopup } from '@/components';
import { usePopups } from '@/hooks';
import { CoinBalance } from '@iota/iota-sdk/client';
import {
    COINS_QUERY_REFETCH_INTERVAL,
    COINS_QUERY_STALE_TIME,
    filterAndSortTokenBalances,
    useSortedCoinsByCategories,
} from '@iota/core';

function MyCoins(): React.JSX.Element {
    const { openPopup, closePopup } = usePopups();
    const account = useCurrentAccount();
    const activeAccountAddress = account?.address;

    const { data: coinBalances } = useIotaClientQuery(
        'getAllBalances',
        { owner: activeAccountAddress! },
        {
            enabled: !!activeAccountAddress,
            staleTime: COINS_QUERY_STALE_TIME,
            refetchInterval: COINS_QUERY_REFETCH_INTERVAL,
            select: filterAndSortTokenBalances,
        },
    );
    const { recognized, unrecognized } = useSortedCoinsByCategories(coinBalances ?? []);

    function openSendTokenPopup(coin: CoinBalance, address: string): void {
        if (coinBalances) {
            openPopup(
                <SendCoinPopup
                    coin={coin}
                    senderAddress={address}
                    onClose={closePopup}
                    coins={coinBalances}
                />,
            );
        }
    }

    return (
        <div className="flex w-2/3 flex-col items-center space-y-2">
            <h3>My Coins:</h3>
            {recognized?.map((coin, index) => {
                return (
                    <CoinItem
                        key={index}
                        coinType={coin.coinType}
                        balance={BigInt(coin.totalBalance)}
                        onClick={() => openSendTokenPopup(coin, account?.address ?? '')}
                    />
                );
            })}
            <span>Unrecognized coins</span>
            {unrecognized?.map((coin, index) => {
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

export default MyCoins;
