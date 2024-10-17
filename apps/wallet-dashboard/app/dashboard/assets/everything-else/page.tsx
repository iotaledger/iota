// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import { IotaObjectData, getNetwork } from '@iota/iota-sdk/client';
import { VirtualList } from '@/components/index';
import { hasDisplayData, useGetOwnedObjects } from '@iota/core';
import { useCurrentAccount, useIotaClientContext } from '@iota/dapp-kit';

function EverythingElsePage(): JSX.Element {
    const account = useCurrentAccount();
    const { data, fetchNextPage, hasNextPage, isFetchingNextPage } = useGetOwnedObjects(
        account?.address,
    );

    const { network } = useIotaClientContext();
    const { explorer } = getNetwork(network);

    const nonVisualAssets =
        data?.pages
            .flatMap((page) => page.data)
            .filter((asset) => asset.data && asset.data.objectId && !hasDisplayData(asset))
            .map((response) => response.data!) ?? [];

    const virtualItem = (asset: IotaObjectData): JSX.Element => (
        <a href={`${explorer}/object/${asset.objectId}`} target="_blank" rel="noreferrer">
            {asset.objectId}
        </a>
    );

    return (
        <div className="flex h-full w-full flex-col items-center justify-center space-y-4">
            <h1>EVERYTHING ELSE</h1>
            <div className="flex w-1/2">
                <VirtualList
                    items={nonVisualAssets}
                    hasNextPage={hasNextPage}
                    isFetchingNextPage={isFetchingNextPage}
                    fetchNextPage={fetchNextPage}
                    estimateSize={() => 30}
                    render={virtualItem}
                />
            </div>
        </div>
    );
}

export default EverythingElsePage;
