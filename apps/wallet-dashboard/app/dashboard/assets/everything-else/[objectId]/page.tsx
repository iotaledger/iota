// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import { useParams } from 'next/navigation';
import { AssetCard, Button, RouteLink } from '@/components';
import { useCurrentAccount } from '@iota/dapp-kit';
import { hasDisplayData, useGetOwnedObjects } from '@iota/core';

const EverythingElseDetailPage = () => {
    const account = useCurrentAccount();
    const params = useParams();
    const objectId = params.objectId;

    const { data: ownedObjects } = useGetOwnedObjects(account?.address);
    const nonVisualAsset = ownedObjects?.pages
        .flatMap((page) => page.data)
        .find((asset) => asset.data && !hasDisplayData(asset) && asset.data.objectId === objectId);

    const isAssetTransferable =
        !!nonVisualAsset &&
        nonVisualAsset.data?.content?.dataType === 'moveObject' &&
        nonVisualAsset.data?.content?.hasPublicTransfer;

    return (
        <div className="flex h-full w-full flex-col space-y-4 px-40">
            <RouteLink path="/dashboard/assets/everything-else" title="Back" />
            {nonVisualAsset?.data ? (
                <AssetCard key={nonVisualAsset.data.objectId} asset={nonVisualAsset.data} />
            ) : (
                <div className="flex justify-center p-20">Asset not found</div>
            )}
            {isAssetTransferable ? (
                <Button onClick={() => console.log('Send Non Visual Asset')}>Send Asset</Button>
            ) : null}
        </div>
    );
};

export default EverythingElseDetailPage;
