// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import { useParams } from 'next/navigation';
import { AssetCard, Button, RouteLink } from '@/components';
import { isAssetTransferable, useGetObject } from '@iota/core';

const EverythingElseDetailPage = () => {
    const params = useParams();
    const objectId = params.objectId as string;

    const { data: nonVisualAsset } = useGetObject(objectId);

    return (
        <div className="flex h-full w-full flex-col space-y-4 px-40">
            <RouteLink path="/dashboard/assets/everything-else" title="Back" />
            {nonVisualAsset?.data ? (
                <AssetCard key={nonVisualAsset.data.objectId} asset={nonVisualAsset.data} />
            ) : (
                <div className="flex justify-center p-20">Asset not found</div>
            )}
            {isAssetTransferable(nonVisualAsset?.data) ? (
                <Button onClick={() => console.log('Send Non Visual Asset')}>Send Asset</Button>
            ) : null}
        </div>
    );
};

export default EverythingElseDetailPage;
