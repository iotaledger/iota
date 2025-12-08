// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinItem } from '@iota/core';
import { ampli } from '_src/shared/analytics/ampli';
import { type CoinBalance } from '@iota/iota-sdk/client';
import { NANOS_PER_IOTA } from '@iota/iota-sdk/utils';
import { type ReactNode } from 'react';
import { Link } from 'react-router-dom';
import { useShouldOpenInNewTab } from '_src/ui/app/hooks';

type TokenLinkProps = {
    coinBalance: CoinBalance;
    clickableAction?: ReactNode;
    icon?: ReactNode;
};

export function TokenLink({ coinBalance, clickableAction, icon }: TokenLinkProps) {
    const shouldOpenNewTab = useShouldOpenInNewTab();
    const url = `/send?type=${encodeURIComponent(coinBalance.coinType)}`;
    const linkProps = shouldOpenNewTab ? { target: '_blank', rel: 'noreferrer' } : {};
    return (
        <Link
            to={url}
            onClick={() =>
                ampli.selectedCoin({
                    coinType: coinBalance.coinType,
                    totalBalance: Number(BigInt(coinBalance.totalBalance) / NANOS_PER_IOTA),
                })
            }
            key={coinBalance.coinType}
            className="group/coin w-full no-underline"
            {...linkProps}
        >
            <CoinItem
                coinType={coinBalance.coinType}
                balance={BigInt(coinBalance.totalBalance)}
                clickableAction={clickableAction}
                icon={icon}
            />
        </Link>
    );
}
