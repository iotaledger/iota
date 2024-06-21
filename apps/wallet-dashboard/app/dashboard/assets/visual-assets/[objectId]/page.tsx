// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import { useParams } from 'next/navigation';
import { AssetCard, Button, RouteLink } from '@/components';
import { isAssetTransferable, useGetObject } from '@iota/core';

const VisualAssetDetailPage = () => {
    const params = useParams();
    const objectId = params.objectId as string;
    const { data: visualAsset } = useGetObject(objectId);
    const assetIsTransferable = visualAsset?.data ? isAssetTransferable(visualAsset?.data) : false;

    return (
        <div className="flex h-full w-full flex-col space-y-4 px-40">
            <RouteLink path="/dashboard/assets/visual-assets" title="Back" />
            {visualAsset?.data ? (
                <AssetCard key={visualAsset.data.objectId} asset={visualAsset.data} />
            ) : (
                <div className="flex justify-center p-20">Asset not found</div>
            )}
            {assetIsTransferable ? (
                <Button onClick={() => console.log('Send Visual Asset')}>Send Asset</Button>
            ) : null}
        </div>
    );
};

export default VisualAssetDetailPage;
