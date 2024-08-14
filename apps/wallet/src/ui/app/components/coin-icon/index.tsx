// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ImageIcon } from '_app/shared/image-icon';
import { useCoinMetadata } from '@iota/core';
import { Iota, Unstaked } from '@iota/icons';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { cva, type VariantProps } from 'class-variance-authority';

const imageStyle = cva(['rounded-full flex'], {
    variants: {
        size: {
            sm: 'w-6 h-6',
            md: 'w-7.5 h-7.5',
            lg: 'md:w-10 md:h-10 w-8 h-8',
            xl: 'md:w-31.5 md:h-31.5 w-16 h-16 ',
        },
        fill: {
            iota: 'bg-iota',
            iotaPrimary2023: 'bg-iota-primaryBlue2023',
        },
    },
    defaultVariants: {
        size: 'md',
        fill: 'iotaPrimary2023',
    },
});

function IotaCoin() {
    return (
        <Iota className="flex h-full w-full items-center justify-center rounded-full p-1.5 text-body text-white" />
    );
}

interface NonIotaCoinProps {
    coinType: string;
}

function NonIotaCoin({ coinType }: NonIotaCoinProps) {
    const { data: coinMeta } = useCoinMetadata(coinType);
    return (
        <div className="bg-steel flex h-full w-full items-center justify-center rounded-full text-white">
            {coinMeta?.iconUrl ? (
                <ImageIcon
                    src={coinMeta.iconUrl}
                    label={coinMeta.name || coinType}
                    fallback={coinMeta.name || coinType}
                    rounded="full"
                />
            ) : (
                <Unstaked />
            )}
        </div>
    );
}

export interface CoinIconProps extends VariantProps<typeof imageStyle> {
    coinType: string;
}

export function CoinIcon({ coinType, ...styleProps }: CoinIconProps) {
    return (
        <div className={imageStyle(styleProps)}>
            {coinType === IOTA_TYPE_ARG ? <IotaCoin /> : <NonIotaCoin coinType={coinType} />}
        </div>
    );
}
