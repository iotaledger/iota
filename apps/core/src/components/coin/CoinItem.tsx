// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import {
    Card,
    CardAction,
    CardActionType,
    CardBody,
    CardImage,
    CardType,
    ImageType,
} from '@iota/apps-ui-kit';
import { CoinIcon, ImageIconSize } from '../';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { type ReactNode } from 'react';
import { useFormatCoin } from '../../hooks';

interface CoinItemProps {
    coinType: string;
    balance: bigint;
    onClick?: () => void;
    icon?: ReactNode;
    clickableAction?: ReactNode;
    usd?: number;
}

export function CoinItem({
    coinType,
    balance,
    onClick,
    icon,
    clickableAction,
    usd,
}: CoinItemProps): React.JSX.Element {
    const [formatted, symbol, { data: coinMeta }] = useFormatCoin(balance, coinType);
    const isIota = coinType === IOTA_TYPE_ARG;

    return (
        <Card type={CardType.Default} onClick={onClick}>
            <CardImage type={ImageType.BgTransparent}>
                <div className="flex h-10 w-10 items-center justify-center rounded-full">
                    <CoinIcon
                        coinType={coinType}
                        rounded
                        size={ImageIconSize.Small}
                        hasCoinWrapper
                    />
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
