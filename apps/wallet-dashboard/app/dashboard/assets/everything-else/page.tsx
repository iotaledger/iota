// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import { getNetwork, IotaObjectData } from '@iota/iota.js/client';
import { VirtualList } from '@/components/index';
import { useGetNFTs } from '@iota/core';
import { useCurrentAccount, useIotaClientContext } from '@iota/dapp-kit';

function EverythingElsePage(): JSX.Element {
    const account = useCurrentAccount();
    const { network } = useIotaClientContext();
    const { explorer } = getNetwork(network);
    const { data } = useGetNFTs(account?.address);
    const nonVisualAssets = data?.other ?? [];

    const virtualItem = (asset: IotaObjectData): JSX.Element => (
        <a href={`${explorer}/object/${asset.objectId}`} target="_blank" rel="noreferrer">
            {asset.objectId}
        </a>
    );

    return (
        <div className="flex h-full w-full flex-col items-center justify-center space-y-4">
            <h1>EVERYTHING ELSE</h1>
            <div className="flex w-1/2">
                <VirtualList items={nonVisualAssets} estimateSize={() => 30} render={virtualItem} />
            </div>
        </div>
    );
}

export default EverythingElsePage;
