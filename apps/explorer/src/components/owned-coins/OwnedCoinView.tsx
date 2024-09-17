// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFormatCoin } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import clsx from 'clsx';
import { useState } from 'react';

import { CoinIcon } from './CoinIcon';
import { type CoinBalanceVerified } from './OwnedCoins';
import CoinsPanel from './OwnedCoinsPanel';
import { Card, CardBody, Chip, Tooltip } from '@iota/apps-ui-kit';
import { ArrowUp, Warning } from '@iota/ui-icons';

type OwnedCoinViewProps = {
    coin: CoinBalanceVerified;
    id: string;
};

export default function OwnedCoinView({ coin, id }: OwnedCoinViewProps): JSX.Element {
    const isIotaCoin = coin.coinType === IOTA_TYPE_ARG;
    const [areCoinDetailsOpen, setAreCoinDetailsOpen] = useState<boolean>(isIotaCoin);
    const [formattedTotalBalance, symbol] = useFormatCoin(coin.totalBalance, coin.coinType);

    const CARD_BODY: React.ComponentProps<typeof CardBody> = {
        title: symbol,
        subtitle: `${formattedTotalBalance} ${symbol}`,
    };

    const CHIP_PROPS: React.ComponentProps<typeof Chip> = {
        label: `${coin.coinObjectCount} Object` + (coin.coinObjectCount > 1 ? 's' : ''),
        trailingElement: <ArrowUp className={clsx({ 'rotate-180': !areCoinDetailsOpen })} />,
    };
    return (
        <div>
            <Card>
                <div className="rounded-full border border-neutral-92 dark:border-neutral-10">
                    <CoinIcon coinType={coin.coinType} size="lg" />
                </div>
                <div className="mr-auto flex flex-row items-center gap-md">
                    <CardBody {...CARD_BODY} />
                    {!coin.isRecognized && (
                        <Tooltip text="This coin has not been recognized by Iota Foundation.">
                            <Warning />
                        </Tooltip>
                    )}
                </div>
                <div className="whitespace-nowrap">
                    <Chip
                        {...CHIP_PROPS}
                        onClick={() => {
                            setAreCoinDetailsOpen((prev) => !prev);
                        }}
                    />
                </div>
            </Card>
            {areCoinDetailsOpen && (
                <div className="flex flex-col gap-xs px-md--rs pb-md--rs pt-xs--rs">
                    <CoinsPanel id={id} coinType={coin.coinType} />
                </div>
            )}
        </div>
    );
}
