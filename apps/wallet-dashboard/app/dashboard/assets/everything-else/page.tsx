// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import { IotaObjectData } from '@iota/iota.js/client';
import { AssetCard, VirtualList } from '@/components/index';
import { hasDisplayData, useGetOwnedObjects } from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';

function EverythingElsePage(): JSX.Element {
    const account = useCurrentAccount();
    const { data } = useGetOwnedObjects(account?.address);
    const nonVisualAssets =
        data?.pages
            .flatMap((page) => page.data)
            .filter((asset) => asset.data && asset.data.objectId && !hasDisplayData(asset))
            .map((response) => response.data!) ?? [];

    const virtualItem = (asset: IotaObjectData): JSX.Element => (
        <AssetCard key={asset.objectId} asset={asset} />
    );

    return (
        <div className="flex h-full w-full flex-col items-center justify-center space-y-4">
            <h1>EVERYTHING ELSE</h1>

            <div className="flex w-1/2">
                <VirtualList
                    items={nonVisualAssets}
                    estimateSize={() => 130}
                    render={virtualItem}
                />
            </div>
        </div>
    );
}

export default EverythingElsePage;
