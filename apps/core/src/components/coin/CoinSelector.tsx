// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Select, SelectOption } from '@iota/apps-ui-kit';
import { CoinBalance } from '@iota/iota-sdk/client';
import { useFormatCoin } from '../../hooks';
import { CoinIcon } from './CoinIcon';
import { ImageIconSize } from '../icon';

interface CoinSelectorProps {
    activeCoinType: string;
    coins: CoinBalance[];
    onClick: (coinType: string) => void;
}

export function CoinSelector({
    activeCoinType = IOTA_TYPE_ARG,
    coins,
    onClick,
}: CoinSelectorProps) {
    const activeCoin = coins?.find(({ coinType }) => coinType === activeCoinType) ?? coins?.[0];
    const initialValue = activeCoin?.coinType;
    const coinsOptions: SelectOption[] =
        coins?.map((coin) => ({
            id: coin.coinType,
            renderLabel: () => <CoinSelectOption coin={coin} />,
        })) || [];

    return (
        <Select
            label="Select Coins"
            value={initialValue}
            options={coinsOptions}
            onValueChange={(coinType) => {
                onClick(coinType);
            }}
        />
    );
}

function CoinSelectOption({ coin: { coinType, totalBalance } }: { coin: CoinBalance }) {
    const [formatted, symbol, { data: coinMeta }] = useFormatCoin(totalBalance, coinType);
    const isIota = coinType === IOTA_TYPE_ARG;

    return (
        <div className="flex w-full flex-row items-center justify-between">
            <div className="flex flex-row items-center gap-x-md">
                <div className="flex h-6 w-6 items-center justify-center">
                    <CoinIcon size={ImageIconSize.Small} coinType={coinType} rounded />
                </div>
                <span className="text-body-lg text-neutral-10">
                    {isIota ? (coinMeta?.name || '').toUpperCase() : coinMeta?.name || symbol}
                </span>
            </div>
            <span className="text-label-lg text-neutral-60">
                {formatted} {symbol}
            </span>
        </div>
    );
}
