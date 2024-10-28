// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCoinMetadata } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { IotaLogoMark } from '@iota/ui-icons';
import cx from 'clsx';
import { ImageIcon, ImageIconSize } from '../ImageIcon';

interface NonIotaCoinProps {
    coinType: string;
    size?: ImageIconSize;
    rounded?: boolean;
}

function NonIotaCoin({ coinType, size = ImageIconSize.Full, rounded }: NonIotaCoinProps) {
    const { data: coinMeta } = useCoinMetadata(coinType);
    return (
        <div className="flex h-full w-full items-center justify-center rounded-full bg-neutral-96 text-neutral-10 dark:bg-neutral-20 dark:text-neutral-100">
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
        <div className="flex h-full w-full items-center justify-center border border-shader-neutral-light-8 bg-neutral-100 text-neutral-10 dark:bg-neutral-0 dark:text-neutral-100">
            <IotaLogoMark className={cx(size)} />
        </div>
    ) : (
        <NonIotaCoin rounded={rounded} size={size} coinType={coinType} />
    );
}
