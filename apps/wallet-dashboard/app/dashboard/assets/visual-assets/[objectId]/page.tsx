// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import Image from 'next/image';
import { useParams } from 'next/navigation';
import { RouteLink } from '@/components';
import { HARCODED_VISUAL_ASSETS } from '@/lib/mocks';

const VisualAssetDetailPage = () => {
    const params = useParams();
    const objectId = params.objectId;

    const asset = HARCODED_VISUAL_ASSETS.find((a) => a.objectId === objectId);

    return (
        <div className="flex h-full w-full flex-col space-y-4 px-40">
            <RouteLink path="/dashboard/assets/visual-assets" title="Back" />
            {asset ? (
                <div className="flex h-full w-full flex-col items-center justify-center space-y-4">
                    <h1>Asset Details</h1>
                    {asset.display && asset.display.data && asset.display.data.image && (
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
            ) : (
                <div className="flex justify-center p-20">Asset not found</div>
            )}
        </div>
    );
};

export default VisualAssetDetailPage;
