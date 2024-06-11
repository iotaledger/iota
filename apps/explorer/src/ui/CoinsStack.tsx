// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCoinMetadata } from '@iota/core';
import { Iota, Unstaked } from '@iota/icons';
import { type CoinMetadata } from '@iota/iota.js/client';
import clsx from 'clsx';

import { Image } from '~/ui/image/Image';

interface CoinIconProps {
    coinMetadata?: CoinMetadata | null;
}

function CoinIcon({ coinMetadata }: CoinIconProps): JSX.Element {
    if (coinMetadata?.symbol === 'IOTA') {
        return <Iota className="h-2.5 w-2.5" />;
    }

    if (coinMetadata?.iconUrl) {
        return <Image rounded="full" alt={coinMetadata?.description} src={coinMetadata?.iconUrl} />;
    }

    return <Unstaked className="h-2.5 w-2.5" />;
}

interface CoinProps {
    type: string;
}

export function Coin({ type }: CoinProps): JSX.Element {
    const { data: coinMetadata } = useCoinMetadata(type);
    const { symbol, iconUrl } = coinMetadata || {};

    return (
        <span
            className={clsx(
                'relative flex h-5 w-5 items-center justify-center rounded-xl text-white',
                (!coinMetadata || symbol !== 'IOTA') &&
                    'bg-gradient-to-r from-gradient-blue-start to-gradient-blue-end',
                symbol === 'IOTA' && 'bg-iota',
                iconUrl && 'bg-gray-40',
            )}
        >
            <CoinIcon coinMetadata={coinMetadata} />
        </span>
    );
}

export interface CoinsStackProps {
    coinTypes: string[];
}

export function CoinsStack({ coinTypes }: CoinsStackProps): JSX.Element {
    return (
        <div className="flex">
            {coinTypes.map((coinType, index) => (
                <div key={index} className={clsx(index !== 0 && '-ml-1')}>
                    <Coin type={coinType} />
                </div>
            ))}
        </div>
    );
}
