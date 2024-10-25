// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React, { useCallback } from 'react';
import { useParams } from 'next/navigation';
import { AssetCard, Button, RouteLink, SendAssetPopup } from '@/components';
import { isAssetTransferable, useGetObject } from '@iota/core';
import { usePopups } from '@/hooks';
import { useCurrentAccount } from '@iota/dapp-kit';

const VisualAssetDetailPage = () => {
    const params = useParams();
    const objectId = params.objectId as string;
    const { data: asset } = useGetObject(objectId);
    const activeAccount = useCurrentAccount();

    const { openPopup, closePopup } = usePopups();

    const showSendAssetPopup = useCallback(() => {
        if (asset?.data) {
            openPopup(<SendAssetPopup asset={asset?.data} onClose={closePopup} />);
        }
    }, [asset, openPopup, closePopup]);

    const assetIsTransferable = asset?.data ? isAssetTransferable(asset?.data) : false;

    return (
        <div className="flex h-full w-full flex-col space-y-4 px-40">
            <RouteLink path="/dashboard/assets/visual-assets" title="Back" />
            {asset?.data ? (
                <AssetCard key={asset.data.objectId} asset={asset.data} />
            ) : (
                <div className="flex justify-center p-20">Asset not found</div>
            )}
            {assetIsTransferable && activeAccount ? (
                <Button onClick={showSendAssetPopup}>Send Asset</Button>
            ) : null}
        </div>
    );
};

export default VisualAssetDetailPage;
