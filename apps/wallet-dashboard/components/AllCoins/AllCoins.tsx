// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetAllCoins } from '@mysten/core/src/hooks/useGetAllCoins';
import { useCurrentAccount } from '@mysten/dapp-kit';
import { SUI_TYPE_ARG } from '@mysten/sui.js/utils';
import { usePopups } from '@/hooks';
import type { CoinStruct } from '@mysten/sui.js/client';
import { Button } from '@/components/index';
import { SendCoinPopup } from '../Popup';

export const AllCoins = () => {
    const account = useCurrentAccount();
    const { data } = useGetAllCoins(SUI_TYPE_ARG, account?.address);
    const { openPopup, closePopup } = usePopups();

    const openSendTokenPopup = (coin: CoinStruct) => {
        openPopup(<SendCoinPopup coin={coin} onClose={closePopup} />);
    };

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
                        <Button onClick={() => openSendTokenPopup(coin)}>Send</Button>
                    </div>
                );
            })}
        </div>
    );
};
