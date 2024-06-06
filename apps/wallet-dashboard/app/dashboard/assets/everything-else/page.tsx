// Copyright (c) 2024 IOTA Stiftung
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { HARDCODED_NON_VISUAL_ASSETS } from '@/lib/mocks';
import React from 'react';
import { IotaObjectData } from '@iota/iota.js/client';
import { AssetCard, VirtualList } from '@/components/index';

function EverythingElsePage(): JSX.Element {
    const virtualItem = (asset: IotaObjectData): JSX.Element => (
        <AssetCard key={asset.objectId} asset={asset} />
    );

    return (
        <div className="flex h-full w-full flex-col items-center justify-center space-y-4">
            <h1>EVERYTHING ELSE</h1>

            <div className="flex w-1/2">
                <VirtualList
                    items={HARDCODED_NON_VISUAL_ASSETS}
                    estimateSize={() => 130}
                    render={virtualItem}
                />
            </div>
        </div>
    );
}

export default EverythingElsePage;
