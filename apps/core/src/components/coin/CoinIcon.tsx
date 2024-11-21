// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { useCoinMetadata } from '../../hooks';
import { IotaLogoMark } from '@iota/ui-icons';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { ImageIcon, ImageIconSize } from '../icon';
import cx from 'clsx';

interface NonIotaCoinProps {
    coinType: string;
    size?: ImageIconSize;
    rounded?: boolean;
}

function NonIotaCoin({ coinType, size = ImageIconSize.Full, rounded }: NonIotaCoinProps) {
    const { data: coinMeta } = useCoinMetadata(coinType);
    return (
        <div className="flex h-full w-full items-center justify-center rounded-full bg-neutral-96 dark:bg-neutral-92">
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
        <div className={cx(size, 'text-neutral-10')}>
            <IotaLogoMark className="h-full w-full" />
        </div>
    ) : (
        <NonIotaCoin rounded={rounded} size={size} coinType={coinType} />
    );
}
