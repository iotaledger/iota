// Copyright (c) 2024 IOTA Stiftung
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetAllCoins } from '@iota/core/src/hooks/useGetAllCoins';
import { useCurrentAccount } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota.js/utils';

export const AllCoins = () => {
    const account = useCurrentAccount();
    const { data } = useGetAllCoins(IOTA_TYPE_ARG, account?.address);

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
