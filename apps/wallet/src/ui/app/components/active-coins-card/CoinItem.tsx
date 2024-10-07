// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinIcon } from '_components';
import { useFormatCoin } from '@iota/core';
import { type ReactNode } from 'react';
import {
    Card,
    CardAction,
    CardActionType,
    CardBody,
    CardImage,
    CardType,
    ImageType,
} from '@iota/apps-ui-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

interface CoinItemProps {
    coinType: string;
    balance: bigint;
    usd?: number;
    clickableAction?: ReactNode;
    icon?: ReactNode;
}

export function CoinItem({ coinType, balance, usd, clickableAction, icon }: CoinItemProps) {
    const [formatted, symbol, { data: coinMeta }] = useFormatCoin(balance, coinType);
    const isIota = coinType === IOTA_TYPE_ARG;

    return (
        <Card type={CardType.Default}>
            <CardImage type={ImageType.BgTransparent}>
                <div className="flex h-10 w-10 items-center justify-center rounded-full border border-shader-neutral-light-8 text-neutral-10">
                    <CoinIcon coinType={coinType} rounded />
                </div>
            </CardImage>
            <CardBody
                title={isIota ? (coinMeta?.name || '').toUpperCase() : coinMeta?.name || symbol}
                subtitle={symbol}
                clickableAction={clickableAction}
                icon={icon}
            />
            <CardAction
                type={CardActionType.SupportingText}
                title={`${formatted} ${symbol}`}
                subtitle={usd?.toLocaleString('en-US')}
            />
        </Card>
    );
}
