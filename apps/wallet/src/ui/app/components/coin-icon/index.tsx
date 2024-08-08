// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ImageIcon } from '_app/shared/image-icon';
import { useCoinMetadata } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota.js/utils';
import { IotaLogoMark, PlaceholderReplace } from '@iota/ui-icons';

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

export interface CoinIconProps {
    coinType: string;
}

export function CoinIcon({ coinType, ...styleProps }: CoinIconProps) {
    return (
        <div className="flex h-full w-full items-center justify-center p-1.5">
            {coinType === IOTA_TYPE_ARG ? (
                <IotaLogoMark className="h-full w-full" />
            ) : (
                <NonIotaCoin coinType={coinType} />
            )}
        </div>
    );
}
