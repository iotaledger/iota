// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import { useParams } from 'next/navigation';
import { AssetCard, RouteLink } from '@/components';
import { useCurrentAccount } from '@iota/dapp-kit';
import { useGetObject } from '@iota/core';

const VisualAssetDetailPage = () => {
    const params = useParams();
    const account = useCurrentAccount();
    const objectId = params.objectId as string;
    const { data: asset } = useGetObject(objectId);

    if (!account) return;

    return (
        <div className="flex h-full w-full flex-col space-y-4 px-40">
            <RouteLink path="/dashboard/assets/everything-else" title="Back" />
            {asset?.data ? (
                <AssetCard key={asset?.data?.objectId} asset={asset?.data} />
            ) : (
                <div className="flex justify-center p-20">Asset not found</div>
            )}
        </div>
    );
};

export default VisualAssetDetailPage;
