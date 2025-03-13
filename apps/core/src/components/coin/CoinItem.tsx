// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

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
import { CoinFormat, useFormatCoin } from '../../hooks';

interface CoinItemProps {
    coinType: string;
    balance: bigint;
    onClick?: () => void;
    icon?: ReactNode;
    clickableAction?: ReactNode;
    usd?: number;
    format?: CoinFormat;
}

export function CoinItem({
    coinType,
    balance,
    onClick,
    icon,
    clickableAction,
    usd,
    format,
}: CoinItemProps): React.JSX.Element {
    const [formatted, symbol, { data: coinMeta }] = useFormatCoin({ balance, coinType, format });
    const isIota = coinType === IOTA_TYPE_ARG;

    return (
        <Card type={CardType.Default} onClick={onClick}>
            <CardImage type={ImageType.BgTransparent}>
                <div className="flex h-10 w-10 items-center justify-center rounded-full border border-shader-neutral-light-8 text-neutral-10 dark:text-neutral-92">
                    <CoinIcon coinType={coinType} rounded size={ImageIconSize.Small} />
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
