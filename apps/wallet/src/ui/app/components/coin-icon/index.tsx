// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ImageIcon, ImageIconSize } from '_app/shared/image-icon';
import { useCoinMetadata } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { IotaLogoMark } from '@iota/ui-icons';
import cx from 'clsx';

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
        <NonIotaCoin rounded={rounded} size={size} coinType={coinType} />
    );
}
