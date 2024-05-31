// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { HARDCODED_NON_VISUAL_ASSETS } from '@/lib/mocks';
import React from 'react';
import { AssetCard, VirtualList } from '@/components/index';
import { useRouter } from 'next/navigation';
import { SuiObjectData } from '@mysten/sui.js/client';

function EverythingElsePage(): JSX.Element {
    const router = useRouter();

    const virtualItem = (asset: SuiObjectData): JSX.Element => (
        <AssetCard key={asset.objectId} asset={asset} />
    );

    const handleClick = (objectId: string) => {
        router.push(`/dashboard/assets/everything-else/${objectId}`);
    };

    return (
        <div className="flex h-full w-full flex-col items-center justify-center space-y-4">
            <h1>EVERYTHING ELSE</h1>
            <div className="flex w-1/2">
                <VirtualList
                    items={HARDCODED_NON_VISUAL_ASSETS}
                    estimateSize={() => 140}
                    render={virtualItem}
                    handleOnClick={(asset) => handleClick(asset.objectId)}
                />
            </div>
        </div>
    );
}

export default EverythingElsePage;
