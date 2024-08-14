// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ImageIcon } from '_app/shared/image-icon';
import { useCoinMetadata } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { IotaLogoMark } from '@iota/ui-icons';

interface NonIotaCoinProps {
    coinType: string;
}

function NonIotaCoin({ coinType }: NonIotaCoinProps) {
    const { data: coinMeta } = useCoinMetadata(coinType);
    return (
        <div className="flex h-full w-full items-center justify-center rounded-full">
            <ImageIcon
                src={coinMeta?.iconUrl}
                label={coinMeta?.name || coinType}
                fallback={coinMeta?.name || coinType}
            />
        </div>
    );
}

export interface CoinIconProps {
    coinType: string;
}

export function CoinIcon({ coinType }: CoinIconProps) {
    return coinType === IOTA_TYPE_ARG ? (
        <div className="h-full w-full p-1">
            <IotaLogoMark className="h-full w-full" />
        </div>
    ) : (
        <NonIotaCoin coinType={coinType} />
    );
}
