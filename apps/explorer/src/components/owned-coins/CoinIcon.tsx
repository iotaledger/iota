// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCoinMetadata } from '@iota/core';
import { Unstaked } from '@iota/icons';
import { IotaLogoMark as Iota } from '@iota/ui-icons';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { cva, type VariantProps } from 'class-variance-authority';

import { ImageIcon } from '~/components/ui';

const imageStyle = cva(['flex rounded-2xl'], {
    variants: {
        size: {
            sm: 'w-6 h-6',
            md: 'w-7.5 h-7.5',
            lg: 'md:w-10 md:h-10 w-8 h-8',
            xl: 'md:w-31.5 md:h-31.5 w-16 h-16 ',
        },
    },
    defaultVariants: {
        size: 'md',
    },
});

function IotaCoin(): JSX.Element {
    return (
        <Iota className="flex h-full w-full rounded-2xl p-xxxs text-neutral-0 dark:text-neutral-100" />
    );
}

type NonIotaCoinProps = {
    coinType: string;
};

function NonIotaCoin({ coinType }: NonIotaCoinProps): JSX.Element {
    const { data: coinMeta } = useCoinMetadata(coinType);
    return (
        <div className="flex h-full w-full items-center justify-center rounded-2xl">
            {coinMeta?.iconUrl ? (
                <ImageIcon
                    size="sm"
                    src={coinMeta.iconUrl}
                    label={coinMeta.name || coinType}
                    fallback={coinMeta.name || coinType}
                    circle
                />
            ) : (
                <div className="flex h-full w-full items-center justify-center rounded-2xl">
                    <Unstaked className="h-2.5 w-2.5" />
                </div>
            )}
        </div>
    );
}

interface CoinIconProps extends VariantProps<typeof imageStyle> {
    coinType: string;
}

export function CoinIcon({ coinType, ...styleProps }: CoinIconProps): JSX.Element {
    return (
        <div className={imageStyle(styleProps)}>
            {coinType === IOTA_TYPE_ARG ? <IotaCoin /> : <NonIotaCoin coinType={coinType} />}
        </div>
    );
}
