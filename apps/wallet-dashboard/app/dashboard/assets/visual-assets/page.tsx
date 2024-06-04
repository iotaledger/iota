// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import { SuiObjectData } from '@mysten/sui.js/client';
import { AssetCard, VirtualList } from '@/components/index';

function VisualAssetsPage(): JSX.Element {
    const virtualItem = (asset: SuiObjectData): JSX.Element => (
        <AssetCard key={asset.objectId} asset={asset} />
    );

    return (
        <div className="flex h-full w-full flex-col items-center justify-center space-y-4">
            <h1>VISUAL ASSETS</h1>
            <div className="flex w-1/2">
                <VirtualList
                    items={HARCODED_VISUAL_ASSETS}
                    estimateSize={() => 130}
                    render={virtualItem}
                />
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
