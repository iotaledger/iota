// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetAllCoins } from '@mysten/core/src/hooks/useGetAllCoins';
import { useCurrentAccount } from '@mysten/dapp-kit';
import { SUI_TYPE_ARG } from '@mysten/sui.js/utils';

export const AllCoins = () => {
    const account = useCurrentAccount();
    const { data } = useGetAllCoins(SUI_TYPE_ARG, account?.address);

    return (
        <div>
            Coins:
            {data?.map((coin) => {
                return (
                    <div key={coin.coinObjectId}>
                        {coin.balance} - {coin.coinObjectId}
                    </div>
                );
            })}
        </div>
    );
};
