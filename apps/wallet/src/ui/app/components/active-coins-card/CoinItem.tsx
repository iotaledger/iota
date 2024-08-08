// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinIcon } from '_components/coin-icon';
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

interface CoinItemProps {
    coinType: string;
    balance: bigint;
    isActive?: boolean;
    usd?: number;
    centerAction?: ReactNode;
    subtitle?: string;
}

export function CoinItem({
    coinType,
    balance,
    isActive,
    usd,
    centerAction,
    subtitle,
}: CoinItemProps) {
    const [formatted, symbol, { data: coinMeta }] = useFormatCoin(balance, coinType);
    const isIota = coinType.includes('iota');

    return (
        <Card type={CardType.Default} isDisabled={!isActive}>
            <CardImage type={ImageType.BgTransparent}>
                <div className="flex h-10 w-10 items-center justify-center rounded-full border border-shader-neutral-light-8  text-neutral-10 ">
                    <CoinIcon coinType={coinType} />
                </div>
            </CardImage>
            <CardBody
                title={isIota ? (coinMeta?.name || '').toUpperCase() : coinMeta?.name || symbol}
            />
            <CardAction
                type={CardActionType.SupportingText}
                title={`${formatted} ${symbol}`}
                subtitle={usd?.toLocaleString('en-US')}
            />
        </Card>
        // <div className="hover:bg-iota/10 flex w-full items-center justify-center gap-2.5 rounded py-3 pl-1.5 pr-2">
        //     <CoinIcon coinType={coinType} size={isActive ? 'sm' : 'md'} />
        //     <div className="flex flex-1 items-center justify-between gap-1.5">
        //         <div className="max-w-token-width">
        //             <Text variant="body" color="gray-90" weight="semibold" truncate>
        //                 {coinMeta?.name || symbol} {isActive ? 'available' : ''}
        //             </Text>
        //             {!isActive && !subtitle ? (
        //                 <div className="mt-1.5">
        //                     <Text variant="subtitle" color="steel-dark" weight="medium">
        //                         {symbol}
        //                     </Text>
        //                 </div>
        //             ) : null}
        //             {subtitle ? (
        //                 <div className="mt-1.5">
        //                     <Text variant="subtitle" color="steel" weight="medium">
        //                         {subtitle}
        //                     </Text>
        //                 </div>
        //             ) : null}
        //         </div>

        //         {centerAction}

        //         <div className="flex flex-row items-center justify-center">
        //             {isActive ? (
        //                 <Text variant="body" color="steel-darker" weight="medium">
        //                     {formatted}
        //                 </Text>
        //             ) : (
        //                 <div data-testid={coinType} className="max-w-token-width">
        //                     <Text variant="body" color="gray-90" weight="medium" truncate>
        //                         {formatted} {symbol}
        //                     </Text>
        //                     {usd && (
        //                         <Text variant="caption" color="steel-dark" weight="medium">
        //                             ${usd.toLocaleString('en-US')}
        //                         </Text>
        //                     )}
        //                 </div>
        //             )}
        //         </div>
        //     </div>
        // </div>
    );
}
