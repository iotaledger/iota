// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCoinMetadata } from '@iota/core';
import { IotaLogoMark } from '@iota/ui-icons';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { cx } from 'class-variance-authority';

import { ImageIcon } from '~/components/ui';

export enum ImageIconSize {
    Small = 'w-5 h-5',
    Medium = 'w-8 h-8',
    Large = 'w-10 h-10',
    Full = 'w-full h-full',
}

interface NonIotaCoinProps {
    coinType: string;
    size?: ImageIconSize;
    rounded?: boolean;
}

function NonIotaCoin({ coinType, size = ImageIconSize.Full, rounded }: NonIotaCoinProps) {
    const { data: coinMeta } = useCoinMetadata(coinType);
    return (
        <div className="flex h-full w-full items-center justify-center rounded-full">
            <ImageIcon
                src={coinMeta?.iconUrl}
                label={coinMeta?.name || coinType}
                fallback={coinMeta?.name || coinType}
                size={size}
                rounded={rounded}
            />
        </div>
    );
}
export interface CoinIconProps {
    coinType: string;
    size?: ImageIconSize;
    rounded?: boolean;
}

export function CoinIcon({ coinType, size = ImageIconSize.Full, rounded }: CoinIconProps) {
    return coinType === IOTA_TYPE_ARG ? (
        <div className={cx(size)}>
            <IotaLogoMark className="h-full w-full" />
        </div>
    ) : (
        <NonIotaCoin rounded={rounded} size={ImageIconSize.Full} coinType={coinType} />
    );
}
