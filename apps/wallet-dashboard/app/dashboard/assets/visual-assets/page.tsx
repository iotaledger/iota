// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { Box } from '@/components/index';
import Image from 'next/image';
import { useCurrentAccount } from '@mysten/dapp-kit';
import { hasDisplayData, useGetOwnedObjects } from '@mysten/core';

function VisualAssetsPage(): JSX.Element {
    const account = useCurrentAccount();
    const { data } = useGetOwnedObjects(account?.address);
    const visualAssets =
        data?.pages
            .flatMap((page) => page.data)
            .filter((asset) => asset.data?.objectId && hasDisplayData(asset)) ?? [];
    const containerRef = React.useRef(null);
    const virtualizer = useVirtualizer({
        count: visualAssets.length,
        getScrollElement: () => containerRef.current,
        estimateSize: () => 130,
    });

    const virtualItems = virtualizer.getVirtualItems();

    return (
        <div className="flex h-full w-full flex-col items-center justify-center space-y-4">
            <h1>VISUAL ASSETS</h1>
            <div className="relative h-[50vh] w-2/3 overflow-auto" ref={containerRef}>
                <div
                    style={{
                        height: `${virtualizer.getTotalSize()}px`,
                        width: '100%',
                        position: 'relative',
                    }}
                >
                    {virtualItems.map((virtualItem) => {
                        const asset = visualAssets[virtualItem.index];
                        return (
                            <div
                                key={virtualItem.key}
                                className="absolute w-full pb-4 pr-4"
                                style={{
                                    position: 'absolute',
                                    top: 0,
                                    left: 0,
                                    width: '100%',
                                    height: `${virtualItem.size}px`,
                                    transform: `translateY(${virtualItem.start}px)`,
                                }}
                            >
                                <Box>
                                    <div className="flex gap-2">
                                        {asset.data?.display &&
                                            asset.data?.display.data &&
                                            asset.data?.display.data.image && (
                                                <Image
                                                    src={asset.data?.display.data.image}
                                                    alt={asset.data?.display.data.name}
                                                    width={80}
                                                    height={40}
                                                />
                                            )}
                                        <div>
                                            <p>Digest: {asset.data?.digest}</p>
                                            <p>Object ID: {asset.data?.objectId}</p>
                                            <p>Version: {asset.data?.version}</p>
                                        </div>
                                    </div>
                                </Box>
                            </div>
                        );
                    })}
                </div>
            </div>
        </div>
    );
}

export default VisualAssetsPage;
