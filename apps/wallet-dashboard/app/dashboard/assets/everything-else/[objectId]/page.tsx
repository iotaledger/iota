// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import { useParams } from 'next/navigation';
import { AssetCard, RouteLink } from '@/components';
import { useCurrentAccount } from '@iota/dapp-kit';
import { useGetNFTs } from '@iota/core';

const VisualAssetDetailPage = () => {
    const params = useParams();
    const account = useCurrentAccount();
    const { data } = useGetNFTs(account?.address);

    if (!account) return;

    const nonVisualAssets = data?.other ?? [];
    const objectId = params.objectId;
    const asset = nonVisualAssets.find((a) => a.objectId === objectId);

    return (
        <div className="flex h-full w-full flex-col space-y-4 px-40">
            <RouteLink path="/dashboard/assets/everything-else" title="Back" />
            {asset ? (
                <AssetCard key={asset.objectId} asset={asset} />
            ) : (
                <div className="flex justify-center p-20">Asset not found</div>
            )}
        </div>
    );
};

export default VisualAssetDetailPage;
