// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import { useParams } from 'next/navigation';
import { AssetCard, RouteLink } from '@/components';
import { useGetNFTs } from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';

const VisualAssetDetailPage = () => {
    const params = useParams();
    const account = useCurrentAccount();
    const { data } = useGetNFTs(account?.address);

    if (!account) return;

    const visualAssets = data?.visual ?? [];
    const objectId = params.objectId;

    const asset = visualAssets.find((a) => a.objectId === objectId);

    return (
        <div className="flex h-full w-full flex-col space-y-4 px-40">
            <RouteLink path="/dashboard/assets/visual-assets" title="Back" />
            {asset ? (
                <AssetCard key={asset.objectId} asset={asset} showSendButton />
            ) : (
                <div className="flex justify-center p-20">Asset not found</div>
            )}
        </div>
    );
};

export default VisualAssetDetailPage;
