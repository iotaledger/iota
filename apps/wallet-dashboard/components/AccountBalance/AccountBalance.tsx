// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

import { useBalance, useFormatCoin } from '@iota/core';

export const AccountBalance = () => {
    const account = useCurrentAccount();
    const address = account?.address;
    const { data: coinBalance, isPending } = useBalance(address!);
    const [formatted, symbol] = useFormatCoin(coinBalance?.totalBalance, IOTA_TYPE_ARG);
    return (
        <div>
            {isPending && <p>Loading...</p>}
            {!isPending && (
                <p>
                    Balance: {formatted} {symbol}
                </p>
            )}
        </div>
    );
};
