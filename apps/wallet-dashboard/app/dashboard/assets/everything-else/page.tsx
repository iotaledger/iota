// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import { IotaObjectData } from '@iota/iota.js/client';
import { AssetCard, VirtualList } from '@/components/index';
import { hasDisplayData, useGetOwnedObjects } from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { useRouter } from 'next/navigation';

function EverythingElsePage(): JSX.Element {
    const account = useCurrentAccount();
    const router = useRouter();
    const { data } = useGetOwnedObjects(account?.address);
    const nonVisualAssets =
        data?.pages
            .flatMap((page) => page.data)
            .filter((asset) => asset.data && asset.data.objectId && !hasDisplayData(asset))
            .map((response) => response.data!) ?? [];

    const virtualItem = (asset: IotaObjectData): JSX.Element => (
        <AssetCard key={asset.objectId} asset={asset} />
    );

    const handleClick = (objectId: string) => {
        router.push(`/dashboard/assets/everything-else/${objectId}`);
    };

    const calculateEstimateSize = (index: number) => {
        const asset = nonVisualAssets[index];
        return asset.display?.data ? 240 : 160;
    };

    return (
        <div className="flex h-full w-full flex-col items-center justify-center space-y-4">
            <h1>EVERYTHING ELSE</h1>
            <div className="flex w-1/2">
                <VirtualList
                    items={nonVisualAssets}
                    estimateSize={calculateEstimateSize}
                    render={virtualItem}
                    onClick={(asset) => handleClick(asset.objectId)}
                />
            </div>
        </div>
    );
}

export default EverythingElsePage;
