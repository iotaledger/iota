// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetAllCoins } from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota.js/utils';
import { SendCoinButton } from '@/components/index';

export const AllCoins = () => {
    const account = useCurrentAccount();
    const { data } = useGetAllCoins(IOTA_TYPE_ARG, account?.address);

    return (
        <div className="space-y-4">
            Coins:
            {data?.map((coin) => {
                return (
                    <div
                        key={coin.coinObjectId}
                        className="flex items-center justify-between gap-4"
                    >
                        {coin.balance} - {coin.coinObjectId}
                        {account?.address ? (
                            <SendCoinButton address={account.address} coin={coin} />
                        ) : null}
                    </div>
                );
            })}
        </div>
    );
};
