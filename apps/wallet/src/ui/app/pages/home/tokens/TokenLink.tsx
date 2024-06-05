// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinItem } from '_components/active-coins-card/CoinItem';
import { ampli } from '_src/shared/analytics/ampli';
import { type CoinBalance } from '@iota/iota.js/client';
import { MICROS_PER_IOTA } from '@iota/iota.js/utils';
import { type ReactNode } from 'react';
import { Link } from 'react-router-dom';

type Props = {
    coinBalance: CoinBalance;
    centerAction?: ReactNode;
    subtitle?: string;
};

export function TokenLink({ coinBalance, centerAction, subtitle }: Props) {
    return (
        <Link
            to={`/send?type=${encodeURIComponent(coinBalance.coinType)}`}
            onClick={() =>
                ampli.selectedCoin({
                    coinType: coinBalance.coinType,
                    totalBalance: Number(BigInt(coinBalance.totalBalance) / MICROS_PER_IOTA),
                })
            }
            key={coinBalance.coinType}
            className="group/coin w-full no-underline"
        >
            <CoinItem
                coinType={coinBalance.coinType}
                balance={BigInt(coinBalance.totalBalance)}
                centerAction={centerAction}
                subtitle={subtitle}
            />
        </Link>
    );
}
