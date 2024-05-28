// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { SuiObjectData } from '@mysten/sui.js/client';
import React from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { Box } from '@/components/index';

function VirtualAssetsPage(): JSX.Element {
    const containerRef = React.useRef(null);
    const virtualizer = useVirtualizer({
        count: VIRTUAL_ASSETS.length,
        getScrollElement: () => containerRef.current,
        estimateSize: () => 130,
    });

    const virtualItems = virtualizer.getVirtualItems();

    return (
        <div className="flex h-full w-full flex-col items-center justify-center space-y-4">
            <h1>VIRTUAL ASSETS</h1>
            <div className="relative h-[50vh] w-2/3 overflow-auto" ref={containerRef}>
                <div
                    style={{
                        height: `${virtualizer.getTotalSize()}px`,
                        width: '100%',
                        position: 'relative',
                    }}
                >
                    {virtualItems.map((virtualItem) => {
                        const asset = VIRTUAL_ASSETS[virtualItem.index];
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
                                    <p>Digest: {asset.digest}</p>
                                    <p>Object ID: {asset.objectId}</p>
                                    <p>Version: {asset.version}</p>
                                </Box>
                            </div>
                        );
                    })}
                </div>
            </div>
        </div>
    );
}
const VIRTUAL_ASSETS: SuiObjectData[] = [
    {
        digest: 'dh3bxjGDzm62bdidFFehtaajwqBSaKFdm8Ujr23J51xy',
        objectId: '0x9303adf2c711dcc239rbd78c0d0666666df06e2b3a35837',
        version: '286606',
    },
    {
        digest: 'dh3bxjGDzm62bdidFFehtaajwqBSaKFdm8Ujr23J51xy',
        objectId: '0x9303adf2c711dcc239rbd78c0d0666666df06e2b3a35837',
        version: '286606',
    },
    {
        digest: 'dh3bxjGDzm62bdidFFehtaajwqBSaKFdm8Ujr23J51xy',
        objectId: '0x9303adf2c711dcc239rbd78c0d0666666df06e2b3a35837',
        version: '286606',
    },
    {
        digest: 'dh3bxjGDzm62bdidFFehtaajwqBSaKFdm8Ujr23J51xy',
        objectId: '0x9303adf2c711dcc239rbd78c0d0666666df06e2b3a35837',
        version: '286606',
    },
    {
        digest: 'dh3bxjGDzm62bdidFFehtaajwqBSaKFdm8Ujr23J51xy',
        objectId: '0x9303adf2c711dcc239rbd78c0d0666666df06e2b3a35837',
        version: '286606',
    },
    {
        digest: 'dh3bxjGDzm62bdidFFehtaajwqBSaKFdm8Ujr23J51xy',
        objectId: '0x9303adf2c711dcc239rbd78c0d0666666df06e2b3a35837',
        version: '286606',
    },
    {
        digest: 'dh3bxjGDzm62bdidFFehtaajwqBSaKFdm8Ujr23J51xy',
        objectId: '0x9303adf2c711dcc239rbd78c0d0666666df06e2b3a35837',
        version: '286606',
    },
    {
        digest: 'dh3bxjGDzm62bdidFFehtaajwqBSaKFdm8Ujr23J51xy',
        objectId: '0x9303adf2c711dcc239rbd78c0d0666666df06e2b3a35837',
        version: '286606',
    },
];

export default VirtualAssetsPage;
