// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFormatCoin, ImageIconSize, CoinIcon } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import clsx from 'clsx';
import { useState } from 'react';
import { type CoinBalanceVerified } from './OwnedCoins';
import { CoinsPanel } from './OwnedCoinsPanel';
import {
    Card,
    CardAction,
    CardActionType,
    CardBody,
    CardImage,
    Divider,
    ImageType,
} from '@iota/apps-ui-kit';
import { ArrowUp, RecognizedBadge } from '@iota/ui-icons';

type OwnedCoinViewProps = {
    coin: CoinBalanceVerified;
    id: string;
};

export function OwnedCoinView({ coin, id }: OwnedCoinViewProps): JSX.Element {
    const isIotaCoin = coin.coinType === IOTA_TYPE_ARG;
    const [areCoinDetailsOpen, setAreCoinDetailsOpen] = useState<boolean>(isIotaCoin);
    const [formattedTotalBalance, symbol] = useFormatCoin(coin.totalBalance, coin.coinType);

    const CARD_BODY: React.ComponentProps<typeof CardBody> = {
        title: symbol,
        subtitle: `${formattedTotalBalance} ${symbol}`,
        icon: coin.isRecognized && <RecognizedBadge className="h-4 w-4 text-primary-40" />,
    };
    return (
        <div
            data-testid="ownedcoinlabel"
            className={clsx(
                'rounded-xl border',
                areCoinDetailsOpen ? 'border-shader-neutral-light-8' : 'border-transparent',
            )}
        >
            <Card onClick={() => setAreCoinDetailsOpen((prev) => !prev)}>
                <CardImage type={ImageType.Placeholder}>
                    <div className="flex h-10 w-10 items-center justify-center rounded-full border border-shader-neutral-light-8 text-neutral-10">
                        <CoinIcon coinType={coin.coinType} size={ImageIconSize.Small} />
                    </div>
                </CardImage>
                <CardBody {...CARD_BODY} isTextTruncated />
                <CardAction
                    type={CardActionType.Button}
                    onClick={() => setAreCoinDetailsOpen((prev) => !prev)}
                    title={`${coin.coinObjectCount} Object` + (coin.coinObjectCount > 1 ? 's' : '')}
                    icon={<ArrowUp className={clsx({ 'rotate-180': !areCoinDetailsOpen })} />}
                    iconAfterText
                />
            </Card>
            {areCoinDetailsOpen && (
                <>
                    <div className="flex justify-center">
                        <div className="w-9/12">
                            <Divider />
                        </div>
                    </div>
                    <div className="flex flex-col gap-xs px-md--rs pb-md--rs pt-sm--rs">
                        <CoinsPanel id={id} coinType={coin.coinType} />
                    </div>
                </>
            )}
        </div>
    );
}
