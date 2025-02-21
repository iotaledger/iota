// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { useCoinMetadata, ImageIcon, ImageIconSize } from '../../';
import { IotaLogoMark } from '@iota/apps-ui-icons';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import cx from 'clsx';

interface NonIotaCoinProps {
    coinType: string;
    size?: ImageIconSize;
    rounded?: boolean;
}

function NonIotaCoin({ coinType, size = ImageIconSize.Full, rounded }: NonIotaCoinProps) {
    const { data: coinMeta } = useCoinMetadata(coinType);
    return (
        <div className="flex h-full w-full items-center justify-center rounded-full bg-neutral-96 dark:bg-neutral-12">
            <ImageIcon
                src={coinMeta?.iconUrl}
                label={coinMeta?.name || coinType}
                fallback={coinMeta?.name || coinType}
                size={coinMeta?.iconUrl ? ImageIconSize.Full : size}
                rounded={rounded}
            />
        </div>
    );
}
export interface CoinIconProps {
    coinType: string;
    size?: ImageIconSize;
    rounded?: boolean;
    hasCoinWrapper?: boolean;
}

export function CoinIcon({
    coinType,
    size = ImageIconSize.Full,
    rounded,
    hasCoinWrapper,
}: CoinIconProps) {
    const Component = hasCoinWrapper ? CoinIconWrapper : React.Fragment;
    const coinWrapperProps = hasCoinWrapper ? { hasBorder: true, size: ImageIconSize.Large } : {};

    return coinType === IOTA_TYPE_ARG ? (
        <Component {...coinWrapperProps}>
            <div className={cx(size, 'text-neutral-10 dark:text-neutral-92')}>
                <IotaLogoMark className="h-full w-full" />
            </div>
        </Component>
    ) : (
        <NonIotaCoin rounded={rounded} size={size} coinType={coinType} />
    );
}
type CoinIconWrapperProps = React.PropsWithChildren<Pick<CoinIconProps, 'size'>> & {
    hasBorder?: boolean;
};
export function CoinIconWrapper({ children, size, hasBorder }: CoinIconWrapperProps) {
    return (
        <div
            className={cx(
                size,
                hasBorder && 'border border-shader-neutral-light-8',
                'flex items-center justify-center rounded-full bg-neutral-100',
            )}
        >
            {children}
        </div>
    );
}
