// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFormatCoin, ImageIconSize, CoinIcon } from '@iota/core';
import {
    Card,
    CardAction,
    CardActionType,
    CardBody,
    CardImage,
    CardType,
    ImageType,
} from '@iota/apps-ui-kit';

interface TxnAmountProps {
    amount: string | number | bigint;
    coinType: string;
    subtitle: string;
    approximation?: boolean;
}

// dont show amount if it is 0
// This happens when a user sends a transaction to self;
export function TxnAmount({ amount, coinType, subtitle, approximation }: TxnAmountProps) {
    const [formatAmount, symbol] = useFormatCoin(Math.abs(Number(amount)), coinType);

    return Number(amount) !== 0 ? (
        <Card type={CardType.Filled}>
            <CardImage type={ImageType.BgSolid}>
                <div className="flex h-full w-full items-center justify-center rounded-full border border-shader-neutral-light-8 text-neutral-10 dark:text-neutral-92">
                    <CoinIcon coinType={coinType} rounded size={ImageIconSize.Small} />
                </div>
            </CardImage>
            <CardBody
                title={`${approximation ? '~' : ''}${formatAmount} ${symbol}`}
                subtitle={subtitle}
            />
            <CardAction type={CardActionType.SupportingText} />
        </Card>
    ) : null;
}
