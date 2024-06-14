// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import { IotaObjectData } from '@iota/iota.js/client';
import { AssetCard, VirtualList } from '@/components/index';
import { useCurrentAccount } from '@iota/dapp-kit';
import { useGetNFTs } from '@iota/core';
import { useRouter } from 'next/navigation';

function VisualAssetsPage(): JSX.Element {
    const account = useCurrentAccount();
    const router = useRouter();
    const { data } = useGetNFTs(account?.address);
    const visualAssets = data?.visual ?? [];

    const virtualItem = (asset: IotaObjectData): JSX.Element => (
        <AssetCard key={asset.objectId} asset={asset} />
    );

    const handleClick = (objectId: string) => {
        router.push(`/dashboard/assets/visual-assets/${objectId}`);
    };

    return (
        <div className="flex h-full w-full flex-col items-center justify-center space-y-4">
            <h1>VISUAL ASSETS</h1>
            <div className="flex w-1/2">
                <VirtualList
                    items={visualAssets}
                    estimateSize={() => 130}
                    render={virtualItem}
                    onClick={(asset) => handleClick(asset.objectId)}
                />
            </div>
        </div>
    );
}

export default VisualAssetsPage;
