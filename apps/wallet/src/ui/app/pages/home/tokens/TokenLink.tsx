// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinItem } from '_components/active-coins-card/CoinItem';
import { ampli } from '_src/shared/analytics/ampli';
import { type CoinBalance } from '@iota/iota-sdk/client';
import { NANO_PER_IOTA } from '@iota/iota-sdk/utils';
import { type ReactNode } from 'react';
import { Link } from 'react-router-dom';

type TokenLinkProps = {
    coinBalance: CoinBalance;
    clickableAction?: ReactNode;
};

export function TokenLink({ coinBalance, clickableAction }: TokenLinkProps) {
    return (
        <Link
            to={`/send?type=${encodeURIComponent(coinBalance.coinType)}`}
            onClick={() =>
                ampli.selectedCoin({
                    coinType: coinBalance.coinType,
                    totalBalance: Number(BigInt(coinBalance.totalBalance) / NANO_PER_IOTA),
                })
            }
            key={coinBalance.coinType}
            className="group/coin w-full no-underline"
        >
            <CoinItem
                coinType={coinBalance.coinType}
                balance={BigInt(coinBalance.totalBalance)}
                clickableAction={clickableAction}
            />
        </Link>
    );
}
