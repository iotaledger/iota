// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import { IotaObjectData } from '@iota/iota-sdk/client';
import { AssetCard, VirtualList } from '@/components/index';
import { useCurrentAccount } from '@iota/dapp-kit';
import { hasDisplayData, useGetOwnedObjects } from '@iota/core';
import { useRouter } from 'next/navigation';

function VisualAssetsPage(): JSX.Element {
    const account = useCurrentAccount();
    const router = useRouter();
    const {
        data: ownedObjects,
        fetchNextPage,
        hasNextPage,
        isFetchingNextPage,
    } = useGetOwnedObjects(account?.address);

    const visualAssets =
        ownedObjects?.pages
            .flatMap((page) => page.data)
            .filter((asset) => asset.data && asset.data.objectId && hasDisplayData(asset))
            .map((response) => response.data!) ?? [];

    const virtualItem = (asset: IotaObjectData): JSX.Element => <AssetCard asset={asset} />;

    const handleClick = (objectId: string) => {
        router.push(`/dashboard/assets/visual-assets/${objectId}`);
    };

    return (
        <div className="flex h-full w-full flex-col items-center justify-center space-y-4">
            <h1>VISUAL ASSETS</h1>
            <div className="flex w-1/2">
                <VirtualList
                    items={visualAssets}
                    hasNextPage={hasNextPage}
                    isFetchingNextPage={isFetchingNextPage}
                    fetchNextPage={fetchNextPage}
                    estimateSize={() => 180}
                    render={virtualItem}
                    onClick={(asset) => handleClick(asset.objectId)}
                />
            </div>
        </div>
    );
}

export default VisualAssetsPage;
