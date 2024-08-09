// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ImageIcon } from '_app/shared/image-icon';
import { useCoinMetadata } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota.js/utils';
import { IotaLogoMark, PlaceholderReplace } from '@iota/ui-icons';
import { cva, type VariantProps } from 'class-variance-authority';

const imageStyle = cva(['flex h-full w-full items-center justify-center p-1.5'], {
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
interface NonIotaCoinProps {
    coinType: string;
}

function NonIotaCoin({ coinType }: NonIotaCoinProps) {
    const { data: coinMeta } = useCoinMetadata(coinType);
    return (
        <>
            {coinMeta?.iconUrl ? (
                <div className="flex h-full w-full items-center justify-center rounded-full ">
                    <ImageIcon
                        src={coinMeta.iconUrl}
                        label={coinMeta.name || coinType}
                        fallback={coinMeta.name || coinType}
                        rounded="full"
                    />
                </div>
            ) : (
                <PlaceholderReplace className="h-full w-full" />
            )}
        </>
    );
}

export interface CoinIconProps extends VariantProps<typeof imageStyle> {
    coinType: string;
}

export function CoinIcon({ coinType, ...styleProps }: CoinIconProps) {
    return (
        <div className={imageStyle(styleProps)}>
            {coinType === IOTA_TYPE_ARG ? (
                <IotaLogoMark className="h-full w-full" />
            ) : (
                <NonIotaCoin coinType={coinType} />
            )}
        </div>
    );
}
