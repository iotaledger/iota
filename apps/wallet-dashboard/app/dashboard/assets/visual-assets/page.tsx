// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { SuiObjectData } from '@mysten/sui.js/client';
import React from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { Box } from '@/components/index';
import Image from 'next/image';

function VisualAssetsPage(): JSX.Element {
    const containerRef = React.useRef(null);
    const virtualizer = useVirtualizer({
        count: HARCODED_VISUAL_ASSETS.length,
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
                        const asset = HARCODED_VISUAL_ASSETS[virtualItem.index];
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
                                        {asset.display &&
                                            asset.display.data &&
                                            asset.display.data.image && (
                                                <Image
                                                    src={asset.display.data.image}
                                                    alt={asset.display.data.name}
                                                    width={80}
                                                    height={40}
                                                />
                                            )}
                                        <div>
                                            <p>Digest: {asset.digest}</p>
                                            <p>Object ID: {asset.objectId}</p>
                                            <p>Version: {asset.version}</p>
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

const HARCODED_VISUAL_ASSETS: SuiObjectData[] = [
    {
        digest: 'dh3bxjGDzm62bdidFFehtaajwqBSaKFdm8Ujr23J51xy',
        objectId: '0x9303adf2c711dcc239rbd78c0d0666666df06e2b3a35837',
        version: '286606',
        display: {
            data: {
                name: 'IOTA',
                // Update next.config.js to include the image domain in the list of domains
                image: 'https://d315pvdvxi2gex.cloudfront.net/528399e23c1bb7b14cced0b89.png',
            },
        },
    },
    {
        digest: 'dh3bxjGDzm62bdidFFehtaajwqBSaKFdm8Ujr23J51xy',
        objectId: '0x9303adf2c711dcc239rbd78c0d0666666df06e2b3a35837',
        version: '286606',
        display: {
            data: {
                name: 'IOTA',
                image: 'https://d315pvdvxi2gex.cloudfront.net/528399e23c1bb7b14cced0b89.png',
            },
        },
    },
    {
        digest: 'dh3bxjGDzm62bdidFFehtaajwqBSaKFdm8Ujr23J51xy',
        objectId: '0x9303adf2c711dcc239rbd78c0d0666666df06e2b3a35837',
        version: '286606',
        display: {
            data: {
                name: 'IOTA',
                image: 'https://d315pvdvxi2gex.cloudfront.net/528399e23c1bb7b14cced0b89.png',
            },
        },
    },
    {
        digest: 'dh3bxjGDzm62bdidFFehtaajwqBSaKFdm8Ujr23J51xy',
        objectId: '0x9303adf2c711dcc239rbd78c0d0666666df06e2b3a35837',
        version: '286606',
        display: {
            data: {
                name: 'IOTA',
                image: 'https://d315pvdvxi2gex.cloudfront.net/528399e23c1bb7b14cced0b89.png',
            },
        },
    },
    {
        digest: 'dh3bxjGDzm62bdidFFehtaajwqBSaKFdm8Ujr23J51xy',
        objectId: '0x9303adf2c711dcc239rbd78c0d0666666df06e2b3a35837',
        version: '286606',
        display: {
            data: {
                name: 'IOTA',
                image: 'https://d315pvdvxi2gex.cloudfront.net/528399e23c1bb7b14cced0b89.png',
            },
        },
    },
    {
        digest: 'dh3bxjGDzm62bdidFFehtaajwqBSaKFdm8Ujr23J51xy',
        objectId: '0x9303adf2c711dcc239rbd78c0d0666666df06e2b3a35837',
        version: '286606',
        display: {
            data: {
                name: 'IOTA',
                image: 'https://d315pvdvxi2gex.cloudfront.net/528399e23c1bb7b14cced0b89.png',
            },
        },
    },
    {
        digest: 'dh3bxjGDzm62bdidFFehtaajwqBSaKFdm8Ujr23J51xy',
        objectId: '0x9303adf2c711dcc239rbd78c0d0666666df06e2b3a35837',
        version: '286606',
        display: {
            data: {
                name: 'IOTA',
                image: 'https://d315pvdvxi2gex.cloudfront.net/528399e23c1bb7b14cced0b89.png',
            },
        },
    },
    {
        digest: 'dh3bxjGDzm62bdidFFehtaajwqBSaKFdm8Ujr23J51xy',
        objectId: '0x9303adf2c711dcc239rbd78c0d0666666df06e2b3a35837',
        version: '286606',
        display: {
            data: {
                name: 'IOTA',
                image: 'https://d315pvdvxi2gex.cloudfront.net/528399e23c1bb7b14cced0b89.png',
            },
        },
    },
];

export default VisualAssetsPage;
