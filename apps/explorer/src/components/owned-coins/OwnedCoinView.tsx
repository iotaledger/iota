// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFormatCoin } from '@iota/core';
import { ArrowShowAndHideRight12, Warning16 } from '@iota/icons';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Text } from '@iota/ui';
import * as Collapsible from '@radix-ui/react-collapsible';
import clsx from 'clsx';
import { useState } from 'react';

import { Banner, Tooltip } from '~/components/ui';
import { ampli } from '~/lib/utils';
import { CoinIcon } from './CoinIcon';
import { type CoinBalanceVerified } from './OwnedCoins';
import CoinsPanel from './OwnedCoinsPanel';

type OwnedCoinViewProps = {
    coin: CoinBalanceVerified;
    id: string;
};

export default function OwnedCoinView({ coin, id }: OwnedCoinViewProps): JSX.Element {
    const isIotaCoin = coin.coinType === IOTA_TYPE_ARG;
    const [open, setOpen] = useState(isIotaCoin);
    const [formattedTotalBalance, symbol] = useFormatCoin(coin.totalBalance, coin.coinType);

    return (
        <Collapsible.Root open={open} onOpenChange={setOpen}>
            <Collapsible.Trigger
                data-testid="ownedcoinlabel"
                className={clsx(
                    'mt-1 flex w-full items-center rounded-lg bg-opacity-5 p-2 text-left hover:bg-hero-darkest hover:bg-opacity-5',
                    open ? 'rounded-b-none bg-hero-darkest pt-3' : 'rounded-b-lg',
                )}
            >
                <div className="flex w-[45%] items-center gap-1 truncate">
                    <ArrowShowAndHideRight12
                        width={12}
                        className={clsx('text-gray-60', open && 'rotate-90 transform')}
                    />

                    <div className="flex items-center gap-3 truncate">
                        <div className="w-6">
                            <CoinIcon coinType={coin.coinType} size="sm" />
                        </div>
                        <Text color="steel-darker" variant="body/medium" truncate>
                            {symbol}
                        </Text>
                    </div>

                    {!coin.isRecognized && (
                        <Tooltip
                            tip="This coin has not been recognized by Iota Foundation."
                            onOpen={() =>
                                ampli.activatedTooltip({
                                    tooltipLabel: 'unrecognizedCoinWarning',
                                })
                            }
                        >
                            <Banner variant="warning" icon={null} border spacing="sm">
                                <Warning16 />
                            </Banner>
                        </Tooltip>
                    )}
                </div>

                <div className="flex w-[25%] pl-2">
                    <Text
                        color={coin.isRecognized ? 'steel-darker' : 'gray-60'}
                        variant="body/medium"
                    >
                        {coin.coinObjectCount}
                    </Text>
                </div>

                <div className="flex w-[30%] items-center gap-1 truncate pl-1">
                    <Text
                        color={coin.isRecognized ? 'steel-darker' : 'gray-60'}
                        variant="bodySmall/medium"
                        truncate
                    >
                        {formattedTotalBalance}
                    </Text>
                    <Text color="steel" variant="subtitleSmallExtra/normal" truncate>
                        {symbol}
                    </Text>
                </div>
            </Collapsible.Trigger>

            <Collapsible.Content>
                <div className="flex flex-col gap-1 rounded-bl-lg rounded-br-lg bg-gray-40 p-3">
                    <CoinsPanel id={id} coinType={coin.coinType} />
                </div>
            </Collapsible.Content>
        </Collapsible.Root>
    );
}
